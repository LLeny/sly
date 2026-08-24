#include "rom_list_model.h"

#include "sly/sly.hpp"

#include <QDirIterator>
#include <QFile>
#include <QFileInfo>
#include <QFutureWatcher>
#include <QImage>
#include <QPainter>
#include <QSet>
#include <QtConcurrentRun>

#include <algorithm>
#include <cstddef>
#include <cstdint>
#include <exception>
#include <limits>
#include <optional>
#include <span>
#include <string_view>
#include <utility>

namespace {
constexpr double KIBIBYTE = 1024.0;
constexpr double MEBIBYTE = KIBIBYTE * 1024.0;

QString format_size(quint64 bytes) {
    if (bytes >= static_cast<quint64>(MEBIBYTE)) {
        return QStringLiteral("%1 MiB").arg(static_cast<double>(bytes) / MEBIBYTE, 0, 'f', 1);
    }

    return QStringLiteral("%1 KiB").arg(static_cast<double>(bytes) / KIBIBYTE, 0, 'f', 0);
}

QImage fit_wheel(QImage image) {
    if (image.isNull()) {
        return {};
    }

    image = image.convertToFormat(QImage::Format_ARGB32);
    int left = image.width();
    int top = image.height();
    int right = -1;
    int bottom = -1;

    for (int y = 0; y < image.height(); ++y) {
        const auto* pixels = reinterpret_cast<const QRgb*>(image.constScanLine(y));
        for (int x = 0; x < image.width(); ++x) {
            if (qAlpha(pixels[x]) <= 8) {
                continue;
            }
            left = std::min(left, x);
            top = std::min(top, y);
            right = std::max(right, x);
            bottom = std::max(bottom, y);
        }
    }

    if (right < left || bottom < top) {
        return {};
    }

    const auto cropped = image.copy(left, top, right - left + 1, bottom - top + 1);
    const auto scaled =
        cropped.scaled(RomListModel::BANNER_SIZE, Qt::KeepAspectRatio, Qt::SmoothTransformation);

    QImage banner(RomListModel::BANNER_SIZE, QImage::Format_ARGB32_Premultiplied);
    banner.fill(Qt::transparent);
    QPainter painter(&banner);
    painter.drawImage(
        (banner.width() - scaled.width()) / 2, (banner.height() - scaled.height()) / 2, scaled
    );
    return banner;
}

std::optional<sly::CartridgeInfo>
read_cartridge_info(const sly::SystemDescriptor system, const QString& path) {
    QFile rom(path);
    if (!rom.open(QIODevice::ReadOnly)) {
        return std::nullopt;
    }

    const auto bytes = rom.readAll();
    const auto rom_bytes = std::span<const std::uint8_t>(
        reinterpret_cast<const std::uint8_t*>(bytes.constData()),
        static_cast<std::size_t>(bytes.size())
    );

    try {
        return system.inspect(rom_bytes);
    } catch (const std::exception&) {
        return std::nullopt;
    }
}

QImage fetch_wheel(std::size_t system_index, const QString& path) {
    QFile rom(path);
    if (!rom.open(QIODevice::ReadOnly)) {
        return {};
    }

    const auto bytes = rom.readAll();
    const auto rom_bytes = std::span<const std::uint8_t>(
        reinterpret_cast<const std::uint8_t*>(bytes.constData()),
        static_cast<std::size_t>(bytes.size())
    );

    try {
        const auto system = sly::systems().at(system_index);
        const auto wheel = system.fetch_wheel_artwork(rom_bytes);
        if (wheel.size() > static_cast<std::size_t>(std::numeric_limits<int>::max())) {
            return {};
        }
        return fit_wheel(QImage::fromData(wheel.data(), static_cast<int>(wheel.size())));
    } catch (const std::exception&) {
        return {};
    }
}
} // namespace

RomListModel::RomListModel(QObject* parent) : QAbstractTableModel(parent) {}

int RomListModel::rowCount(const QModelIndex& parent) const {
    return parent.isValid() ? 0 : static_cast<int>(entries_.size());
}

int RomListModel::columnCount(const QModelIndex& parent) const {
    return parent.isValid() ? 0 : static_cast<int>(Column::Size) + 1;
}

QVariant RomListModel::data(const QModelIndex& index, const int role) const {
    if (!index.isValid() || index.row() < 0 || index.row() >= entries_.size()) {
        return {};
    }

    const auto& entry = entries_.at(index.row());
    const auto column = static_cast<Column>(index.column());

    if (role == Qt::ToolTipRole) {
        return entry.path;
    }

    if (role == Qt::TextAlignmentRole) {
        if (column == Column::Banner) {
            return QVariant::fromValue(Qt::AlignCenter);
        }
        if (column == Column::Size) {
            return QVariant::fromValue(Qt::AlignRight | Qt::AlignVCenter);
        }
        return QVariant::fromValue(Qt::AlignLeft | Qt::AlignVCenter);
    }

    if (role == SORT_ROLE) {
        switch (column) {
            case Column::Banner:
            case Column::Title:
                return entry.title.toCaseFolded();
            case Column::FileName:
                return entry.file_name.toCaseFolded();
            case Column::Region:
                return entry.region_name.toCaseFolded();
            case Column::Size:
                return QVariant::fromValue(entry.file_size);
        }
    }

    if (role == Qt::DecorationRole && column == Column::Banner) {
        return entry.banner;
    }

    if (role != Qt::DisplayRole) {
        return {};
    }

    switch (column) {
        case Column::Banner:
            return {};
        case Column::Title:
            return entry.title;
        case Column::FileName:
            return entry.file_name;
        case Column::Region:
            return entry.region_name;
        case Column::Size:
            return format_size(entry.file_size);
    }

    return {};
}

QVariant RomListModel::headerData(
    const int section, const Qt::Orientation orientation, const int role
) const {
    if (orientation != Qt::Horizontal || role != Qt::DisplayRole) {
        return {};
    }

    switch (static_cast<Column>(section)) {
        case Column::Banner:
            return tr("Banner");
        case Column::Title:
            return tr("Title");
        case Column::FileName:
            return tr("File");
        case Column::Region:
            return tr("Region");
        case Column::Size:
            return tr("Size");
    }
    return {};
}

void RomListModel::set_entries(QVector<RomEntry> entries) {
    beginResetModel();
    entries_ = std::move(entries);
    endResetModel();
    load_banners();
}

void RomListModel::load_banners() {
    for (int row = 0; row < entries_.size(); ++row) {
        const auto system_index = entries_.at(row).system_index;
        const auto path = entries_.at(row).path;
        auto* watcher = new QFutureWatcher<QImage>(this);

        connect(watcher, &QFutureWatcher<QImage>::finished, this, [this, watcher, row, path] {
            const auto image = watcher->result();
            watcher->deleteLater();

            if (image.isNull() || row >= entries_.size() || entries_.at(row).path != path) {
                return;
            }

            entries_[row].banner = QPixmap::fromImage(image);
            const auto column = static_cast<int>(Column::Banner);
            const auto banner = index(row, column);
            emit dataChanged(banner, banner, {Qt::DecorationRole});
        });

        watcher->setFuture(QtConcurrent::run([system_index, path] {
            return fetch_wheel(system_index, path);
        }));
    }
}

QVector<RomEntry> RomListModel::entries_from_directory(const QString& root_path) {
    const QDir root(root_path);
    if (!root.exists()) {
        return {};
    }

    QVector<RomEntry> entries;
    for (const auto system : sly::systems()) {
        const auto slug = QString::fromStdString(system.slug());

        QSet<QString> extensions;
        for (const auto& extension : system.extensions()) {
            extensions.insert(QString::fromStdString(extension).toLower());
        }

        QDirIterator files(
            root.filePath(slug), QDir::Files | QDir::Readable, QDirIterator::Subdirectories
        );
        while (files.hasNext()) {
            const QFileInfo file(files.next());
            if (!extensions.contains(file.suffix().toLower())) {
                continue;
            }

            const auto cartridge = read_cartridge_info(system, file.absoluteFilePath());
            if (!cartridge) {
                continue;
            }

            const auto cartridge_title = QString::fromStdString(cartridge->title);
            const auto title =
                cartridge_title.isEmpty() ? file.completeBaseName() : cartridge_title;

            entries.append({
                .system_index = system.index(),
                .title = title,
                .file_name = file.fileName(),
                .path = file.absoluteFilePath(),
                .region_name = QString::fromStdString(cartridge->region_name),
                .file_size = static_cast<quint64>(file.size()),
                .banner = {},
            });
        }
    }

    return entries;
}

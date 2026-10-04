#pragma once

#include <QAbstractTableModel>
#include <QPixmap>
#include <QSize>
#include <QString>
#include <QVector>

#include <cstddef>
#include <cstdint>

struct RomEntry {
    std::size_t system_index{};
    QString title;
    QString file_name;
    QString path;
    QString region_name;
    quint64 file_size{};
    QPixmap banner;
};

class RomListModel final : public QAbstractTableModel {
public:
    enum class Column : std::uint8_t {
        Banner,
        Title,
        FileName,
        Region,
        Size,
    };

    static constexpr QSize BANNER_SIZE{96, 32};
    static constexpr int SORT_ROLE = Qt::UserRole + 1;

    explicit RomListModel(QObject* parent = nullptr);

    int rowCount(const QModelIndex& parent) const override;
    int columnCount(const QModelIndex& parent) const override;
    QVariant data(const QModelIndex& index, int role) const override;
    QVariant headerData(int section, Qt::Orientation orientation, int role) const override;
    void set_entries(QVector<RomEntry> entries);
    [[nodiscard]] const RomEntry* entry_at(int row) const noexcept;

    static QVector<RomEntry> entries_from_directory(const QString& root_path);

    void load_banners();

private:
    QVector<RomEntry> entries_;
};

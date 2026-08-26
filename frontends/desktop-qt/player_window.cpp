#include "player_window.h"

#include <QDir>
#include <QFile>
#include <QFileInfo>
#include <QMediaDevices>
#include <QMenuBar>
#include <QMessageBox>
#include <QSaveFile>
#include <QStandardPaths>

#include <cstdint>
#include <exception>
#include <format>
#include <print>
#include <span>
#include <stdexcept>
#include <utility>

namespace {
sly::Core
load_core(const sly::SystemDescriptor system, const QString& rom_path, std::string& game_id) {
    QFile rom(rom_path);
    if (!rom.open(QIODevice::ReadOnly)) {
        throw sly::Error(std::format("Unable to open ROM: {}", rom.errorString().toStdString()));
    }

    const auto bytes = rom.readAll();
    const auto rom_bytes =
        std::span(reinterpret_cast<const std::uint8_t*>(bytes.constData()), bytes.size());
    game_id = system.rom_sha1(rom_bytes);
    return system.create(rom_bytes);
}

QString data_root() {
    const QString override = qEnvironmentVariable("SLY_DATA_DIR");
    if (!override.isEmpty()) {
        return override;
    }

    const QString data = QStandardPaths::writableLocation(QStandardPaths::GenericDataLocation);
    return data.isEmpty() ? QStringLiteral("sly-data") : QDir(data).filePath("sly");
}
} // namespace

PlayerWindow::PlayerWindow(
    const sly::SystemDescriptor system, QString rom_path, QString title, QWidget* parent
)
    : QMainWindow(parent), core_(load_core(system, rom_path, game_id_)),
      save_directory_(
          QDir(data_root())
              .filePath(
                  QString::fromStdString(
                      std::format("{}/states/{}", core_.cartridge_info().system, game_id_)
                  )
              )
      ) {
    setWindowTitle(title);

    video_widget_ = new VideoWidget(this);
    setCentralWidget(video_widget_);

    QMenuBar* menu_bar = new QMenuBar(this);
    setMenuBar(menu_bar);

    constexpr int INITIAL_SCALE = 2;

    const auto frame = core_.frame();
    resize_video(frame, INITIAL_SCALE);
    video_widget_->set_frame(frame);

    initialize_audio();

    frame_timer_ = new QChronoTimer(core_.frame_interval(), this);
    frame_timer_->setTimerType(Qt::PreciseTimer);
    connect(frame_timer_, &QChronoTimer::timeout, this, [this] {
        core_.run_frame();
        video_widget_->set_frame(core_.frame());
        drain_audio();
    });
    frame_timer_->start();

    auto* saves_menu = menu_bar->addMenu(tr("&Saves"));
    auto* video_menu = menu_bar->addMenu(tr("&Video"));
    auto* scale_menu = video_menu->addMenu(tr("&Scale"));

    const auto add_save_slot = [this, saves_menu](const QString& label, std::string file_name) {
        auto* slot_menu = saves_menu->addMenu(label);
        slot_menu->addAction(tr("Save"), this, [this, file_name] { save_state(file_name); });
        slot_menu->addAction(tr("Load"), this, [this, file_name] { load_state(file_name); });
    };
    add_save_slot(tr("Quick Slot"), "quick.sav");
    for (int slot = 0; slot <= 9; ++slot) {
        add_save_slot(tr("Slot %1").arg(slot), std::format("slot-{}.sav", slot));
    }

    video_menu->addAction(tr("&Fullscreen"), this, [this] {
        if (isFullScreen()) {
            showNormal();
        } else {
            showFullScreen();
        }
    });

    for (int scale = 1; scale <= 4; ++scale) {
        scale_menu->addAction(tr("Scale %1").arg(scale), this, [this, scale] {
            resize_video(core_.frame(), scale);
        });
    }
}

void PlayerWindow::resize_video(const sly::FrameView frame, const int scale) {
    resize(
        static_cast<int>(frame.width) * scale,
        (static_cast<int>(frame.height) * scale) + menuBar()->sizeHint().height()
    );
}

QString PlayerWindow::state_path(const std::string_view file_name) const {
    return QDir(save_directory_)
        .filePath(QString::fromUtf8(file_name.data(), static_cast<qsizetype>(file_name.size())));
}

void PlayerWindow::save_state(const std::string_view file_name) {
    const QString path = state_path(file_name);

    try {
        const auto bytes = core_.save_state();

        const QFileInfo file_info(path);
        if (!QDir().mkpath(file_info.absolutePath())) {
            throw sly::Error(
                std::format(
                    "Unable to create save directory: {}", file_info.absolutePath().toStdString()
                )
            );
        }

        QSaveFile file(path);
        if (!file.open(QIODevice::WriteOnly)) {
            throw sly::Error(file.errorString().toStdString());
        }

        const auto size = static_cast<qint64>(bytes.size());
        if (file.write(reinterpret_cast<const char*>(bytes.data()), size) != size) {
            throw sly::Error(file.errorString().toStdString());
        }
        if (!file.commit()) {
            throw sly::Error(file.errorString().toStdString());
        }

        std::println("Saved state to {}", path.toStdString());
    } catch (const std::exception& error) {
        QMessageBox::critical(
            this,
            tr("Save State Failed"),
            tr("Unable to save the state:\n%1").arg(QString::fromUtf8(error.what()))
        );
    }
}

void PlayerWindow::load_state(const std::string_view file_name) {
    const QString path = state_path(file_name);

    QFile file(path);
    if (!file.exists()) {
        QMessageBox::information(this, tr("Load State"), tr("This slot is empty."));
        return;
    }

    if (!file.open(QIODevice::ReadOnly)) {
        QMessageBox::critical(
            this,
            tr("Load State Failed"),
            tr("Unable to read the state:\n%1").arg(file.errorString())
        );
        return;
    }

    const QByteArray bytes = file.readAll();
    if (file.error() != QFileDevice::NoError) {
        QMessageBox::critical(
            this,
            tr("Load State Failed"),
            tr("Unable to read the state:\n%1").arg(file.errorString())
        );
        return;
    }

    try {
        core_.load_state(
            std::span(
                reinterpret_cast<const std::uint8_t*>(bytes.constData()),
                static_cast<std::size_t>(bytes.size())
            )
        );
        video_widget_->set_frame(core_.frame());
        std::println("Loaded state from {}", path.toStdString());
    } catch (const std::exception& error) {
        QMessageBox::critical(
            this,
            tr("Load State Failed"),
            tr("Unable to load the state:\n%1").arg(QString::fromUtf8(error.what()))
        );
    }
}

void PlayerWindow::initialize_audio() {
    const auto spec = core_.audio_spec();
    if (!spec) {
        std::println("No audio spec available, skipping audio initialization");
        return;
    }

    QAudioFormat format;
    format.setSampleRate(spec->sample_rate);
    format.setChannelCount(spec->channels);
    format.setSampleFormat(QAudioFormat::Int16);

    const auto device = QMediaDevices::defaultAudioOutput();
    if (!device.isFormatSupported(format)) {
        throw std::runtime_error("Audio format not supported");
    }

    audio_sink_ = new QAudioSink(device, format, this);
    audio_device_ = audio_sink_->start();
}

void PlayerWindow::drain_audio() {
    const auto samples = core_.drain_audio();
    if (!audio_device_ || samples.empty()) {
        return;
    }

    audio_device_->write(
        reinterpret_cast<const char*>(samples.data()), samples.size() * sizeof(std::int16_t)
    );
}

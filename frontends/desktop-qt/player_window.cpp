#include "player_window.h"

#include <QFile>
#include <QMediaDevices>
#include <QMenuBar>

#include <cstdint>
#include <format>
#include <print>
#include <span>
#include <utility>

namespace {
sly::Core load_core(const sly::SystemDescriptor system, const QString& rom_path) {
    QFile rom(rom_path);
    if (!rom.open(QIODevice::ReadOnly)) {
        throw sly::Error(std::format("Unable to open ROM: {}", rom.errorString().toStdString()));
    }

    const auto bytes = rom.readAll();
    const auto rom_bytes =
        std::span(reinterpret_cast<const std::uint8_t*>(bytes.constData()), bytes.size());
    return system.create(rom_bytes);
}
} // namespace

PlayerWindow::PlayerWindow(
    const sly::SystemDescriptor system, QString rom_path, QString title, QWidget* parent
)
    : QMainWindow(parent), system_(system), rom_path_(std::move(rom_path)),
      title_(std::move(title)), core_(load_core(system_, rom_path_)) {
    setWindowTitle(title_);

    video_widget_ = new VideoWidget(this);
    setCentralWidget(video_widget_);

    // QMenuBar* menu_bar = new QMenuBar(this);
    // setMenuBar(menu_bar);

    // auto* video_menu = menu_bar->addMenu(tr("&Video"));
    // auto* action = video_menu->addAction(tr("&Fullscreen"));
    // action->setCheckable(true);
    // action->setChecked(system.index() == 0);

    const auto frame = core_.frame();
    setMinimumSize(static_cast<int>(frame.width), static_cast<int>(frame.height));
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
        throw new std::runtime_error("Audio format not supported");
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

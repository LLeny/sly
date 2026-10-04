#include "player_window.h"

#include <QDir>
#include <QFile>
#include <QFileInfo>
#include <QJsonDocument>
#include <QJsonObject>
#include <QMediaDevices>
#include <QMenuBar>
#include <QMessageBox>
#include <QSaveFile>
#include <QStandardPaths>

#include <algorithm>
#include <chrono>
#include <cstdint>
#include <exception>
#include <format>
#include <optional>
#include <print>
#include <span>
#include <stdexcept>

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

constexpr std::uint8_t QUICK_SLOT = 0;

QString data_root() {
    const QString override = qEnvironmentVariable("SLY_DATA_DIR");
    if (!override.isEmpty()) {
        return override;
    }

    const QString data = QStandardPaths::writableLocation(QStandardPaths::GenericDataLocation);
    return data.isEmpty() ? QStringLiteral("sly-data") : QDir(data).filePath("sly");
}

QString data_path(const std::string& relative) {
    return QDir(data_root()).filePath(QString::fromStdString(relative));
}

QString write_file(const QString& path, const std::span<const std::uint8_t> bytes) {
    const QFileInfo file_info(path);
    if (!QDir().mkpath(file_info.absolutePath())) {
        return QStringLiteral("Unable to create directory: %1").arg(file_info.absolutePath());
    }

    QSaveFile file(path);
    const auto size = static_cast<qint64>(bytes.size());
    if (!file.open(QIODevice::WriteOnly) ||
        file.write(reinterpret_cast<const char*>(bytes.data()), size) != size || !file.commit()) {
        return file.errorString();
    }

    return {};
}

std::optional<sly::Button> button_for_key(const int key) {
    using namespace sly;

    switch (key) {
        case Qt::Key_Up:
            return Button::Up;
        case Qt::Key_Down:
            return Button::Down;
        case Qt::Key_Left:
            return Button::Left;
        case Qt::Key_Right:
            return Button::Right;
        case Qt::Key_Y:
        case Qt::Key_Z:
            return Button::B;
        case Qt::Key_X:
            return Button::A;
        case Qt::Key_A:
            return Button::Y;
        case Qt::Key_S:
            return Button::X;
        case Qt::Key_Q:
            return Button::L;
        case Qt::Key_W:
            return Button::R;
        case Qt::Key_Return:
        case Qt::Key_Enter:
            return Button::Start;
        case Qt::Key_Backspace:
            return Button::Select;
        default:
            return std::nullopt;
    }
}
} // namespace

PlayerWindow::PlayerWindow(
    const sly::SystemDescriptor system, QString rom_path, QString title, QWidget* parent
)
    : QMainWindow(parent), core_(load_core(system, rom_path)) {
    setWindowTitle(title);

    const auto cartridge_info = core_.cartridge_info();
    const auto controllers = core_.controllers();
    if (!controllers.empty()) {
        controller_ = controllers.front();
    }
    if (cartridge_info.backup_kind && cartridge_info.backup_capacity > 0) {
        save_ram_path_ = data_path(core_.save_ram_path());
        restore_save_ram();
    }

    video_widget_ = new VideoWidget(this);
    video_widget_->setFocusPolicy(Qt::StrongFocus);
    setCentralWidget(video_widget_);

    QMenuBar* menu_bar = new QMenuBar(this);
    setMenuBar(menu_bar);

    bool initial_fullscreen = false;
    try {
        const auto preferences =
            QJsonDocument::fromJson(QByteArray::fromStdString(sly::settings_frontend("classic")))
                .object();

        scale_ = std::clamp(preferences.value("scale").toInt(2), 1, 4);
        initial_fullscreen = preferences.value("fullscreen").toBool(false);
    } catch (const std::exception& error) {
        QMessageBox::warning(this, tr("Settings error"), QString::fromUtf8(error.what()));
    }

    const auto frame = core_.frame();
    resize_video(frame, scale_);
    video_widget_->set_frame(frame);

    initialize_audio();

    frame_timer_ = new QChronoTimer(core_.frame_interval(), this);
    frame_timer_->setTimerType(Qt::PreciseTimer);
    connect(frame_timer_, &QChronoTimer::timeout, this, [this] {
        poll_gamepad();
        core_.run_frame();
        video_widget_->set_frame(core_.frame());
        drain_audio();
    });
    frame_timer_->start();

    if (!save_ram_path_.isEmpty()) {
        auto* save_ram_timer = new QChronoTimer(std::chrono::seconds(15), this);
        connect(save_ram_timer, &QChronoTimer::timeout, this, &PlayerWindow::persist_save_ram);
        save_ram_timer->start();
    }

    auto* saves_menu = menu_bar->addMenu(tr("&Saves"));
    auto* emulation_menu = menu_bar->addMenu(tr("&Emulation"));
    auto* video_menu = menu_bar->addMenu(tr("&Video"));
    auto* scale_menu = video_menu->addMenu(tr("&Scale"));

    const auto add_save_slot = [this, saves_menu](
                                   const QString& label,
                                   std::uint8_t slot,
                                   const QString& save_text,
                                   const QString& load_text,
                                   const QKeySequence& save_shortcut = {},
                                   const QKeySequence& load_shortcut = {}
                               ) {
        auto* slot_menu = saves_menu->addMenu(label);
        auto* save_action =
            slot_menu->addAction(save_text, this, [this, slot] { save_state(slot); });
        save_action->setShortcut(save_shortcut);
        auto* load_action =
            slot_menu->addAction(load_text, this, [this, slot] { load_state(slot); });
        load_action->setShortcut(load_shortcut);
    };
    add_save_slot(
        tr("Quick Slot"), QUICK_SLOT, tr("Save\tF5"), tr("Load\tF8"), Qt::Key_F5, Qt::Key_F8
    );
    for (int slot = 0; slot <= 9; ++slot) {
        add_save_slot(
            tr("Slot %1").arg(slot), static_cast<std::uint8_t>(slot + 1), tr("Save"), tr("Load")
        );
    }

    auto* pause_action = emulation_menu->addAction(tr("&Pause"));
    pause_action->setCheckable(true);
    pause_action->setShortcut(Qt::Key_Space);
    connect(pause_action, &QAction::toggled, this, &PlayerWindow::set_paused);

    auto* reset_action = emulation_menu->addAction(tr("&Reset"));
    reset_action->setShortcut(QKeySequence(tr("Ctrl+R")));
    connect(reset_action, &QAction::triggered, this, &PlayerWindow::reset_core);

    auto* fullscreen_action = video_menu->addAction(tr("&Fullscreen"), this, [this] {
        if (isFullScreen()) {
            showNormal();
        } else {
            showFullScreen();
        }

        save_preferences();
    });
    fullscreen_action->setShortcut(Qt::Key_F11);
    fullscreen_action->setCheckable(true);
    fullscreen_action->setChecked(initial_fullscreen);

    if (initial_fullscreen) {
        showFullScreen();
    }

    for (int scale = 1; scale <= 4; ++scale) {
        scale_menu->addAction(tr("Scale %1").arg(scale), this, [this, scale] {
            scale_ = scale;
            resize_video(core_.frame(), scale);
            save_preferences();
        });
    }

    video_widget_->setFocus();
}

PlayerWindow::~PlayerWindow() {
    persist_save_ram();
}

void PlayerWindow::keyPressEvent(QKeyEvent* event) {
    if (handle_game_key(event, true)) {
        return;
    }

    if (event->key() == Qt::Key_Escape && isFullScreen()) {
        showNormal();
        save_preferences();
        event->accept();
        return;
    }

    QMainWindow::keyPressEvent(event);
}

void PlayerWindow::keyReleaseEvent(QKeyEvent* event) {
    if (!handle_game_key(event, false)) {
        QMainWindow::keyReleaseEvent(event);
    }
}

bool PlayerWindow::handle_game_key(QKeyEvent* event, const bool pressed) {
    if (event->isAutoRepeat()) {
        return false;
    }

    const auto button = button_for_key(event->key());
    if (!button || !controller_ ||
        std::ranges::find(controller_->buttons, *button) == controller_->buttons.end()) {
        return false;
    }

    keyboard_state_[static_cast<std::size_t>(*button)] = pressed;
    apply_input();
    event->accept();
    return true;
}

bool PlayerWindow::event(QEvent* event) {
    if (event->type() == QEvent::WindowDeactivate) {
        release_input();
    }
    return QMainWindow::event(event);
}

void PlayerWindow::poll_gamepad() {
    const auto state = isActiveWindow() ? gamepad_.poll() : GamepadInput::State{};
    gamepad_state_ = state.buttons;
    apply_input();

    if (state.quick_save) {
        save_state(QUICK_SLOT);
    }
    if (state.quick_load) {
        load_state(QUICK_SLOT);
    }
}

void PlayerWindow::apply_input() {
    if (!controller_) {
        return;
    }
    for (const auto button : controller_->buttons) {
        const auto index = static_cast<std::size_t>(button);
        if (index < keyboard_state_.size()) {
            core_.set_button(
                controller_->port, button, keyboard_state_[index] || gamepad_state_[index]
            );
        }
    }
}

void PlayerWindow::release_input() {
    keyboard_state_.fill(false);
    gamepad_state_.fill(false);
    apply_input();
}

void PlayerWindow::set_paused(const bool paused) {
    release_input();

    if (paused) {
        frame_timer_->stop();

        if (audio_sink_ != nullptr) {
            audio_sink_->suspend();
        }
    } else {
        if (audio_sink_ != nullptr) {
            audio_sink_->resume();
        }

        frame_timer_->start();
        video_widget_->setFocus();
    }
}

void PlayerWindow::reset_core() {
    release_input();
    core_.reset();
    video_widget_->set_frame(core_.frame());
}

void PlayerWindow::save_preferences() {
    const QJsonObject preferences{
        {"scale", scale_},
        {"fullscreen", isFullScreen()},
    };

    try {
        sly::settings_save_frontend(
            "classic", QJsonDocument(preferences).toJson(QJsonDocument::Compact).toStdString()
        );
    } catch (const std::exception& error) {
        QMessageBox::warning(this, tr("Settings error"), QString::fromUtf8(error.what()));
    }
}

void PlayerWindow::resize_video(const sly::FrameView frame, const int scale) {
    resize(
        static_cast<int>(frame.width) * scale,
        (static_cast<int>(frame.height) * scale) + menuBar()->sizeHint().height()
    );
}

QString PlayerWindow::state_path(const std::uint8_t slot) const {
    return data_path(core_.state_path(slot));
}

void PlayerWindow::save_state(const std::uint8_t slot) {
    const QString path = state_path(slot);

    try {
        if (const QString error = write_file(path, core_.save_state()); !error.isEmpty()) {
            throw sly::Error(error.toStdString());
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

void PlayerWindow::load_state(const std::uint8_t slot) {
    const QString path = state_path(slot);

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

void PlayerWindow::restore_save_ram() {
    QFile file(save_ram_path_);
    if (!file.exists()) {
        return;
    }

    const QByteArray bytes = file.open(QIODevice::ReadOnly) ? file.readAll() : QByteArray();
    if (file.error() != QFileDevice::NoError) {
        core_.inhibit_save_ram_persistence();
        std::println(
            stderr,
            "Failed to read save RAM from {}: {}",
            save_ram_path_.toStdString(),
            file.errorString().toStdString()
        );
        return;
    }

    const auto save_ram = std::span(
        reinterpret_cast<const std::uint8_t*>(bytes.constData()),
        static_cast<std::size_t>(bytes.size())
    );
    if (!core_.load_save_ram(save_ram)) {
        std::println(
            stderr,
            "Stored save RAM has the wrong size for this ROM: {}",
            save_ram_path_.toStdString()
        );
    }
}

void PlayerWindow::persist_save_ram() {
    if (save_ram_path_.isEmpty()) {
        return;
    }

    const auto bytes = core_.persistable_save_ram();
    if (bytes.empty()) {
        return;
    }

    if (const QString error = write_file(save_ram_path_, bytes); !error.isEmpty()) {
        std::println(
            stderr,
            "Failed to write save RAM to {}: {}",
            save_ram_path_.toStdString(),
            error.toStdString()
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

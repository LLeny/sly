#pragma once

#include <QAudioSink>
#include <QChronoTimer>
#include <QIODevice>
#include <QKeyEvent>
#include <QMainWindow>
#include <QString>

#include <sly/sly.hpp>

#include <string>
#include <string_view>

#include "gamepad_input.h"
#include "video_widget.h"

class PlayerWindow final : public QMainWindow {
public:
    explicit PlayerWindow(
        sly::SystemDescriptor system, QString rom_path, QString title, QWidget* parent = nullptr
    );
    ~PlayerWindow() override;

protected:
    void keyPressEvent(QKeyEvent* event) override;
    void keyReleaseEvent(QKeyEvent* event) override;
    bool event(QEvent* event) override;

private:
    void resize_video(sly::FrameView frame, int scale);
    QString state_path(std::string_view file_name) const;
    void save_state(std::string_view file_name);
    void load_state(std::string_view file_name);
    void restore_save_ram();
    void persist_save_ram();
    void initialize_audio();
    void drain_audio();
    void poll_gamepad();
    bool handle_game_key(QKeyEvent* event, bool pressed);
    void apply_input();
    void release_input();
    void set_paused(bool paused);
    void reset_core();

    std::string game_id_;
    sly::Core core_;
    QString save_directory_;
    QString save_ram_path_;

    VideoWidget* video_widget_ = nullptr;
    QChronoTimer* frame_timer_ = nullptr;

    QAudioSink* audio_sink_ = nullptr;
    QIODevice* audio_device_ = nullptr;

    GamepadInput gamepad_;
    sly::ButtonState keyboard_state_{};
    sly::ButtonState gamepad_state_{};
};

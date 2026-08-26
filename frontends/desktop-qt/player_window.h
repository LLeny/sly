#pragma once

#include <QAudioSink>
#include <QChronoTimer>
#include <QIODevice>
#include <QMainWindow>
#include <QString>

#include <sly/sly.hpp>

#include <string>
#include <string_view>

#include "video_widget.h"

class PlayerWindow final : public QMainWindow {
public:
    explicit PlayerWindow(
        sly::SystemDescriptor system, QString rom_path, QString title, QWidget* parent = nullptr
    );

private:
    void resize_video(sly::FrameView frame, int scale);
    QString state_path(std::string_view file_name) const;
    void save_state(std::string_view file_name);
    void load_state(std::string_view file_name);
    void initialize_audio();
    void drain_audio();

    std::string game_id_;
    sly::Core core_;
    QString save_directory_;

    VideoWidget* video_widget_ = nullptr;
    QChronoTimer* frame_timer_ = nullptr;

    QAudioSink* audio_sink_ = nullptr;
    QIODevice* audio_device_ = nullptr;
};

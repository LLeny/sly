#pragma once

#include <QChronoTimer>
#include <QMainWindow>
#include <QString>
#include <QAudioSink>
#include <QIODevice>
#include <QByteArray>

#include <sly/sly.hpp>

#include "video_widget.h"

class PlayerWindow final : public QMainWindow {
public:
    explicit PlayerWindow(
        sly::SystemDescriptor system, QString rom_path, QString title, QWidget* parent = nullptr
    );

private:
    void initialize_audio();
    void drain_audio();

    sly::SystemDescriptor system_;
    QString rom_path_;
    QString title_;
    sly::Core core_;

    VideoWidget* video_widget_ = nullptr;
    QChronoTimer* frame_timer_ = nullptr;

    QAudioSink* audio_sink_ = nullptr;
    QIODevice* audio_device_ = nullptr;
    QByteArray audio_buffer_;
};

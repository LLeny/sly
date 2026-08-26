#pragma once

#include <QChronoTimer>
#include <QMainWindow>
#include <QString>

#include <sly/sly.hpp>

#include "video_widget.h"

class PlayerWindow final : public QMainWindow {
public:
    explicit PlayerWindow(
        sly::SystemDescriptor system, QString rom_path, QString title, QWidget* parent = nullptr
    );

private:
    sly::SystemDescriptor system_;
    QString rom_path_;
    QString title_;
    sly::Core core_;
    VideoWidget* video_widget_ = nullptr;
    QChronoTimer* frame_timer_ = nullptr;
};

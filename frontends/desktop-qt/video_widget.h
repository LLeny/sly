#pragma once

#include <QByteArray>
#include <QRhiWidget>
#include <QSize>
#include <rhi/qrhi.h>

#include <sly/sly.hpp>

#include <memory>

class VideoWidget final : public QRhiWidget {
public:
    using QRhiWidget::QRhiWidget;

    void set_frame(sly::FrameView frame);

private:
    void create_resources();
    void reset_gpu_resources();

    QRhi* rhi_ = nullptr;
    std::unique_ptr<QRhiShaderResourceBindings> bindings_;
    std::unique_ptr<QRhiGraphicsPipeline> pipeline_;
    std::unique_ptr<QRhiTexture> texture_;
    std::unique_ptr<QRhiSampler> sampler_;
    QSize frame_size_;
    QByteArray frame_rgba_;
    bool frame_dirty_ = false;

protected:
    void initialize(QRhiCommandBuffer* command_buffer) override;
    void render(QRhiCommandBuffer* command_buffer) override;
    void releaseResources() override;
};

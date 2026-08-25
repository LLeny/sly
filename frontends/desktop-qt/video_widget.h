#pragma once

#include <QRhiWidget>
#include <rhi/qrhi.h>

class VideoWidget final : public QRhiWidget {
public:
    using QRhiWidget::QRhiWidget;

private:
    QRhi* rhi_ = nullptr;
    std::unique_ptr<QRhiShaderResourceBindings> bindings_;
    std::unique_ptr<QRhiGraphicsPipeline> pipeline_;

protected:
    void initialize(QRhiCommandBuffer* command_buffer) override;
    void render(QRhiCommandBuffer* command_buffer) override;
    void releaseResources() override;
};

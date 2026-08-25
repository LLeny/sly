#include "video_widget.h"

#include <QFile>

#include <format>

namespace {
QShader load_shader(const QString& path) {
    QFile file(path);
    if (!file.open(QIODevice::ReadOnly)) {
        throw std::runtime_error(
            std::format("Failed to open shader file: {}", file.errorString().toStdString())
        );
    }

    const auto bytes = file.readAll();
    return QShader::fromSerialized(bytes);
}
} // namespace

void VideoWidget::initialize(QRhiCommandBuffer* command_buffer) {
    if (rhi_ != rhi()) {
        bindings_.reset();
        pipeline_.reset();
        rhi_ = rhi();
    }

    if (pipeline_) {
        return;
    }

    bindings_.reset(rhi_->newShaderResourceBindings());
    if (!bindings_->create()) {
        throw std::runtime_error("Failed to create shader resource bindings");
    }

    pipeline_.reset(rhi_->newGraphicsPipeline());
    pipeline_->setShaderStages({
        {
            QRhiShaderStage::Vertex,
            load_shader(":/shaders/video.vert.qsb"),
        },
        {
            QRhiShaderStage::Fragment,
            load_shader(":/shaders/video.frag.qsb"),
        },
    });

    pipeline_->setShaderResourceBindings(bindings_.get());
    pipeline_->setRenderPassDescriptor(renderTarget()->renderPassDescriptor());

    if (!pipeline_->create()) {
        throw std::runtime_error("Failed to create graphics pipeline");
    }
}

void VideoWidget::render(QRhiCommandBuffer* command_buffer) {
    command_buffer->beginPass(renderTarget(), Qt::black, {1.0f, 0});

    command_buffer->setGraphicsPipeline(pipeline_.get());

    const QSize output_size = renderTarget()->pixelSize();
    command_buffer->setViewport(QRhiViewport(
        0, 0, static_cast<float>(output_size.width()), static_cast<float>(output_size.height())
    ));

    command_buffer->setShaderResources();
    command_buffer->draw(3);
    command_buffer->endPass();
}

void VideoWidget::releaseResources() {
    pipeline_.reset();
    bindings_.reset();
    rhi_ = nullptr;
}

#include "video_widget.h"

#include <QFile>

#include <algorithm>
#include <cstring>
#include <format>
#include <stdexcept>

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

QRhiViewport letterboxed_viewport(const QSize output_size, const QSize frame_size) {
    const float output_width = static_cast<float>(output_size.width());
    const float output_height = static_cast<float>(output_size.height());
    const float frame_width = static_cast<float>(frame_size.width());
    const float frame_height = static_cast<float>(frame_size.height());

    const float scale = std::min(output_width / frame_width, output_height / frame_height);
    const float width = frame_width * scale;
    const float height = frame_height * scale;

    return {
        (output_width - width) * 0.5f,
        (output_height - height) * 0.5f,
        width,
        height,
    };
}
} // namespace

void VideoWidget::set_frame(const sly::FrameView frame) {
    if (!frame) {
        frame_size_ = {};
        frame_rgba_.clear();
        frame_dirty_ = false;
        return;
    }

    frame_size_ = QSize(static_cast<int>(frame.width), static_cast<int>(frame.height));
    frame_rgba_.resize(static_cast<qsizetype>(frame.rgba.size()));
    std::memcpy(frame_rgba_.data(), frame.rgba.data(), frame.rgba.size());
    frame_dirty_ = true;

    update();
}

void VideoWidget::create_resources() {
    if (frame_rgba_.isEmpty()) {
        return;
    }

    if (texture_ && texture_->pixelSize() != frame_size_) {
        pipeline_.reset();
        bindings_.reset();
        texture_.reset();
    }

    if (!texture_) {
        texture_.reset(rhi_->newTexture(QRhiTexture::RGBA8, frame_size_));
        if (!texture_->create()) {
            throw std::runtime_error("Failed to create video texture");
        }
        frame_dirty_ = true;
    }

    if (!sampler_) {
        sampler_.reset(rhi_->newSampler(
            QRhiSampler::Nearest,
            QRhiSampler::Nearest,
            QRhiSampler::None,
            QRhiSampler::ClampToEdge,
            QRhiSampler::ClampToEdge
        ));
        if (!sampler_->create()) {
            throw std::runtime_error("Failed to create video sampler");
        }
    }

    if (!bindings_) {
        bindings_.reset(rhi_->newShaderResourceBindings());
        bindings_->setBindings({
            QRhiShaderResourceBinding::sampledTexture(
                0, QRhiShaderResourceBinding::FragmentStage, texture_.get(), sampler_.get()
            ),
        });
        if (!bindings_->create()) {
            throw std::runtime_error("Failed to create shader resource bindings");
        }
    }

    if (!pipeline_) {
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
}

void VideoWidget::reset_gpu_resources() {
    pipeline_.reset();
    bindings_.reset();
    sampler_.reset();
    texture_.reset();
}

void VideoWidget::initialize(QRhiCommandBuffer*) {
    if (rhi_ != rhi()) {
        reset_gpu_resources();
        rhi_ = rhi();
    }
}

void VideoWidget::render(QRhiCommandBuffer* command_buffer) {
    create_resources();

    QRhiResourceUpdateBatch* updates = nullptr;
    if (texture_ && frame_dirty_) {
        frame_dirty_ = false;
        updates = rhi_->nextResourceUpdateBatch();
        updates->uploadTexture(
            texture_.get(),
            QRhiTextureUploadEntry(0, 0, QRhiTextureSubresourceUploadDescription(frame_rgba_))
        );
    }

    command_buffer->beginPass(renderTarget(), Qt::black, {1.0f, 0}, updates);

    const QSize output_size = renderTarget()->pixelSize();
    if (pipeline_ && !frame_size_.isEmpty() && !output_size.isEmpty()) {
        command_buffer->setGraphicsPipeline(pipeline_.get());

        command_buffer->setViewport(letterboxed_viewport(output_size, frame_size_));

        command_buffer->setShaderResources(bindings_.get());
        command_buffer->draw(3);
    }

    command_buffer->endPass();
}

void VideoWidget::releaseResources() {
    reset_gpu_resources();
    rhi_ = nullptr;
}

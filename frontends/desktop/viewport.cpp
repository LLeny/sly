#include "viewport.h"

#include <rhi/qrhi.h>

namespace {

class ViewportRenderer : public QQuickRhiItemRenderer {
public:
    void initialize(QRhiCommandBuffer* cb) override;
    void synchronize(QQuickRhiItem* item) override;
    void render(QRhiCommandBuffer* cb) override;

private:
    QByteArray m_frame;
    bool m_dirty = false;
};

void ViewportRenderer::initialize(QRhiCommandBuffer*) {}

void ViewportRenderer::synchronize(QQuickRhiItem* item) {
    auto* viewport = static_cast<Viewport*>(item);
    Player* source = viewport->source();
    if (!source) {
        m_frame.clear();
        return;
    }

    m_dirty = source->copyFrameInto(m_frame);
}

void ViewportRenderer::render(QRhiCommandBuffer* cb) {
    QRhiTexture* target = colorTexture();
    if (!m_dirty || !target)
        return;

    const QSize size = target->pixelSize();
    if (m_frame.size() != qsizetype(size.width()) * size.height() * 4)
        return;

    QRhiTextureSubresourceUploadDescription sub(m_frame);
    QRhiTextureUploadDescription desc({0, 0, sub});

    QRhiResourceUpdateBatch* batch = rhi()->nextResourceUpdateBatch();
    batch->uploadTexture(target, desc);
    cb->resourceUpdate(batch);
    m_dirty = false;
}

} // namespace

Viewport::Viewport(QQuickItem* parent) : QQuickRhiItem(parent) {
    setAutoRenderTarget(false);
    setMirrorVertically(true);
}

void Viewport::setSource(Player* source) {
    if (m_source == source)
        return;
    if (m_source)
        m_source->disconnect(this);

    m_source = source;

    if (m_source) {
        connect(m_source, &Player::frameReady, this, [this] { update(); });
        connect(m_source, &Player::loadedChanged, this, &Viewport::adoptFrameSize);
        adoptFrameSize();
    }
    emit sourceChanged();
}

void Viewport::adoptFrameSize() {
    const QSize size = m_source ? m_source->frameSize() : QSize();
    if (size.isEmpty())
        return;
    setFixedColorBufferWidth(size.width());
    setFixedColorBufferHeight(size.height());
    update();
}

QQuickRhiItemRenderer* Viewport::createRenderer() {
    return new ViewportRenderer;
}

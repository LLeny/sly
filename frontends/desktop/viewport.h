#pragma once

#include <QQuickRhiItem>
#include <QtQmlIntegration>

#include "player.h"

class Viewport : public QQuickRhiItem {
    Q_OBJECT
    QML_ELEMENT

    Q_PROPERTY(Player* source READ source WRITE setSource NOTIFY sourceChanged)

public:
    explicit Viewport(QQuickItem* parent = nullptr);

    Player* source() const { return m_source; }

    void setSource(Player* source);

    QQuickRhiItemRenderer* createRenderer() override;

signals:
    void sourceChanged();

private:
    void adoptFrameSize();

    Player* m_source = nullptr;
};

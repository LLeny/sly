#pragma once

#include <QObject>
#include <QSize>
#include <QString>
#include <QUrl>
#include <QtQmlIntegration>

#include <optional>

#include "sly-cxx/src/lib.rs.h"

class Player : public QObject {
    Q_OBJECT

    QML_NAMED_ELEMENT(PlayerCore)

    Q_PROPERTY(QUrl romUrl READ romUrl WRITE setRomUrl NOTIFY romUrlChanged)
    Q_PROPERTY(bool loaded READ isLoaded NOTIFY loadedChanged)
    Q_PROPERTY(QString title READ title NOTIFY loadedChanged)
    Q_PROPERTY(QString error READ error NOTIFY errorChanged)
    Q_PROPERTY(QSize frameSize READ frameSize NOTIFY loadedChanged)

public:
    explicit Player(QObject* parent = nullptr);
    ~Player() override;

    QUrl romUrl() const { return m_romUrl; }

    void setRomUrl(const QUrl& url);

    bool isLoaded() const { return m_core.has_value(); }

    QString title() const { return m_title; }

    QString error() const { return m_error; }

    QSize frameSize() const { return m_frameSize; }

    bool copyFrameInto(QByteArray& out) const;

public slots:
    bool load();
    void unload();
    void runFrame();

signals:
    void romUrlChanged();
    void loadedChanged();
    void errorChanged();
    void frameReady();

private:
    void setError(const QString& message);

    QUrl m_romUrl;
    QString m_title;
    QString m_error;
    QSize m_frameSize;
    std::optional<rust::Box<sly::SlyCore>> m_core;
};

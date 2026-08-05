#include "player.h"

#include <QFile>

Player::Player(QObject* parent) : QObject(parent) {}

Player::~Player() = default;

void Player::setRomUrl(const QUrl& url) {
    if (m_romUrl == url)
        return;
    m_romUrl = url;

    emit romUrlChanged();
}

bool Player::load() {
    unload();

    const QString path = m_romUrl.isLocalFile() ? m_romUrl.toLocalFile() : m_romUrl.toString();
    QFile file(path);
    if (!file.open(QIODevice::ReadOnly)) {
        setError(tr("Cannot open %1: %2").arg(path, file.errorString()));
        return false;
    }
    const QByteArray rom = file.readAll();

    try {
        m_core = sly::create_core(
            sly::System::Snes,
            rust::Slice<const uint8_t>(
                reinterpret_cast<const uint8_t*>(rom.constData()), rom.size()
            )
        );
    } catch (const std::exception& e) {
        setError(tr("Cannot load %1: %2").arg(path, QString::fromUtf8(e.what())));
        return false;
    }

    const sly::CartridgeInfo info = (*m_core)->cartridge_info();
    m_title = QString::fromUtf8(info.title.data(), info.title.size());

    const sly::FrameInfo frame = (*m_core)->frame_info();
    m_frameSize = QSize(int(frame.width), int(frame.height));

    setError(QString());
    emit loadedChanged();
    return true;
}

void Player::runFrame() {
    if (!m_core.has_value())
        return;

    (*m_core)->run_frame();

    emit frameReady();
}

bool Player::copyFrameInto(QByteArray& out) const {
    if (!m_core.has_value())
        return false;

    const qsizetype expected = qsizetype(m_frameSize.width()) * m_frameSize.height() * 4;
    if (expected <= 0)
        return false;
    if (out.size() != expected)
        out.resize(expected);

    return (*m_core)->copy_frame_rgba(
        rust::Slice<uint8_t>(reinterpret_cast<uint8_t*>(out.data()), size_t(expected))
    );
}

void Player::unload() {
    if (!m_core.has_value())
        return;
    m_core.reset();
    m_title.clear();
    m_frameSize = QSize();
    emit loadedChanged();
}

void Player::setError(const QString& message) {
    if (m_error == message)
        return;
    m_error = message;
    emit errorChanged();
}

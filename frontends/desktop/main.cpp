#include <QDir>
#include <QGuiApplication>
#include <QQmlApplicationEngine>
#include <QQuickStyle>
#include <QUrl>

int main(int argc, char* argv[]) {
#if !defined(Q_OS_MACOS) && !defined(Q_OS_WIN) && !defined(Q_OS_IOS)
    if (qEnvironmentVariableIsEmpty("QT_QUICK_CONTROLS_STYLE"))
        QQuickStyle::setStyle(QStringLiteral("Fusion"));
#endif

    QGuiApplication app(argc, argv);

    const QStringList args = QCoreApplication::arguments();
    const QString romsDir = args.size() > 1 ? args.at(1) : QStringLiteral(SLY_ROMS_DIR);

    QQmlApplicationEngine engine;
    engine.setInitialProperties(
        {{QStringLiteral("romsRoot"), QUrl::fromLocalFile(QDir(romsDir).absolutePath())}}
    );
    QObject::connect(
        &engine,
        &QQmlApplicationEngine::objectCreationFailed,
        &app,
        []() { QCoreApplication::exit(-1); },
        Qt::QueuedConnection
    );
    engine.loadFromModule("desktop", "Main");

    return QGuiApplication::exec();
}

#include <QApplication>

#include "main_window.h"

int main(int argc, char* argv[]) {
    QApplication application(argc, argv);
    QApplication::setApplicationName(QStringLiteral("Sly"));
    QApplication::setOrganizationName(QStringLiteral("Sly"));

    MainWindow window;
    window.show();
    return QApplication::exec();
}

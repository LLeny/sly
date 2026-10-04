#include "main_window.h"

#include "player_window.h"
#include "rom_list_model.h"

#include "sly/sly.hpp"

#include <QActionGroup>
#include <QCheckBox>
#include <QDesktopServices>
#include <QDialog>
#include <QDialogButtonBox>
#include <QDir>
#include <QFormLayout>
#include <QHBoxLayout>
#include <QHeaderView>
#include <QIcon>
#include <QLineEdit>
#include <QMenuBar>
#include <QMessageBox>
#include <QPushButton>
#include <QSortFilterProxyModel>
#include <QStyle>
#include <QStyledItemDelegate>
#include <QTableView>
#include <QUrl>

#include <exception>
#include <print>

#define THEMED_ICON(name, fallback)                                                                \
    QIcon::fromTheme(QStringLiteral(name), style()->standardIcon(QStyle::fallback))

namespace {
class RowDelegate final : public QStyledItemDelegate {
public:
    using QStyledItemDelegate::QStyledItemDelegate;

protected:
    void initStyleOption(QStyleOptionViewItem* option, const QModelIndex& index) const override {
        QStyledItemDelegate::initStyleOption(option, index);
        option->state &= ~QStyle::State_HasFocus;
        if (index.column() == static_cast<int>(RomListModel::Column::Banner)) {
            option->decorationAlignment = Qt::AlignCenter;
        }
    }
};

void configure_table(QTableView& table) {
    const auto column_index = [](RomListModel::Column column) { return static_cast<int>(column); };

    table.setAlternatingRowColors(true);
    table.setSelectionBehavior(QAbstractItemView::SelectRows);
    table.setSelectionMode(QAbstractItemView::SingleSelection);
    table.setItemDelegate(new RowDelegate(&table));
    table.setEditTriggers(QAbstractItemView::NoEditTriggers);
    table.setShowGrid(false);
    table.setSortingEnabled(true);
    table.setWordWrap(false);
    table.setIconSize(RomListModel::BANNER_SIZE);
    table.setHorizontalScrollBarPolicy(Qt::ScrollBarAlwaysOff);
    table.setVerticalScrollMode(QAbstractItemView::ScrollPerPixel);
    table.verticalHeader()->hide();
    table.verticalHeader()->setDefaultSectionSize(36);

    auto* header = table.horizontalHeader();
    header->setSectionsMovable(true);
    header->setHighlightSections(false);
    header->setMinimumSectionSize(72);
    header->setSectionResizeMode(column_index(RomListModel::Column::Banner), QHeaderView::Fixed);
    header->setSectionResizeMode(
        column_index(RomListModel::Column::Title), QHeaderView::Interactive
    );
    header->setSectionResizeMode(
        column_index(RomListModel::Column::FileName), QHeaderView::Stretch
    );
    header->setSectionResizeMode(
        column_index(RomListModel::Column::Region), QHeaderView::Interactive
    );
    header->setSectionResizeMode(column_index(RomListModel::Column::Size), QHeaderView::Fixed);

    table.setColumnWidth(
        column_index(RomListModel::Column::Banner), RomListModel::BANNER_SIZE.width() + 16
    );
    table.setColumnWidth(column_index(RomListModel::Column::Title), 220);
    table.setColumnWidth(column_index(RomListModel::Column::Region), 200);
    table.setColumnWidth(column_index(RomListModel::Column::Size), 96);
    table.sortByColumn(column_index(RomListModel::Column::Title), Qt::AscendingOrder);
}
} // namespace

MainWindow::MainWindow(QWidget* parent) : QMainWindow(parent) {
    setWindowTitle(tr("Sly Classic"));
    setMinimumSize(900, 500);
    resize(1180, 720);

    QMenuBar* menu_bar = new QMenuBar(this);
    setMenuBar(menu_bar);

    auto* file_menu = menu_bar->addMenu(tr("&File"));
    auto* cores_menu = menu_bar->addMenu(tr("&Cores"));

    file_menu->addAction(
        THEMED_ICON("document-open", SP_DialogOpenButton), tr("&Load ROM"), this, [] {
            std::println("load rom stub");
        }
    );
    file_menu->addAction(
        THEMED_ICON("application-exit", SP_DialogCloseButton),
        tr("&Quit"),
        QKeySequence::Quit,
        this,
        &QWidget::close
    );

    auto* core_actions = new QActionGroup(cores_menu);
    core_actions->setExclusive(true);

    for (const auto system : sly::systems()) {
        const auto core_name = QString::fromStdString(system.name());

        auto* action = cores_menu->addAction(core_name);
        action->setCheckable(true);
        action->setChecked(system.index() == 0);
        core_actions->addAction(action);

        connect(action, &QAction::triggered, this, [core_name]() {
            std::println("load core stub: {}", core_name.toStdString());
        });
    }

    auto* model = new RomListModel(this);
    file_menu->addAction(tr("&Settings..."), this, [this, model] {
        QDialog dialog(this);
        dialog.setWindowTitle(tr("Settings"));
        auto* layout = new QFormLayout(&dialog);

        auto* key = new QLineEdit(&dialog);
        key->setEchoMode(QLineEdit::Password);
        key->setPlaceholderText(tr("Anonymous"));

        try {
            key->setText(QString::fromStdString(sly::settings_api_key()));
        } catch (const std::exception& error) {
            QMessageBox::warning(this, tr("Settings error"), QString::fromUtf8(error.what()));
        }

        auto* show = new QCheckBox(tr("Show"), &dialog);
        connect(show, &QCheckBox::toggled, key, [key](bool checked) {
            key->setEchoMode(checked ? QLineEdit::Normal : QLineEdit::Password);
        });

        auto* key_row = new QHBoxLayout;
        key_row->addWidget(key);
        key_row->addWidget(show);
        layout->addRow(tr("Artwork key"), key_row);

        auto* buttons =
            new QDialogButtonBox(QDialogButtonBox::Save | QDialogButtonBox::Cancel, &dialog);
        auto* clear = buttons->addButton(tr("Clear"), QDialogButtonBox::ResetRole);
        auto* get_key = buttons->addButton(tr("Get key"), QDialogButtonBox::HelpRole);
        layout->addRow(buttons);

        connect(clear, &QPushButton::clicked, key, &QLineEdit::clear);
        connect(get_key, &QPushButton::clicked, &dialog, [] {
            QDesktopServices::openUrl(QUrl(QStringLiteral("https://art.layle.dev/account")));
        });
        connect(buttons, &QDialogButtonBox::rejected, &dialog, &QDialog::reject);
        connect(buttons, &QDialogButtonBox::accepted, &dialog, [&dialog, key, model] {
            try {
                sly::settings_set_api_key(key->text().toStdString());
                model->load_banners();
                dialog.accept();
            } catch (const std::exception& error) {
                QMessageBox::warning(
                    &dialog, QObject::tr("Settings error"), QString::fromUtf8(error.what())
                );
            }
        });

        dialog.exec();
    });

    auto* proxy = new QSortFilterProxyModel(this);
    proxy->setSourceModel(model);
    proxy->setSortCaseSensitivity(Qt::CaseInsensitive);
    proxy->setSortRole(RomListModel::SORT_ROLE);

    auto* table = new QTableView(this);
    table->setModel(proxy);
    configure_table(*table);
    setCentralWidget(table);

    connect(
        table, &QTableView::doubleClicked, this, [this, proxy, model](const QModelIndex& index) {
            const auto source_index = proxy->mapToSource(index);
            const auto* entry = model->entry_at(source_index.row());
            if (entry == nullptr) {
                return;
            }

            try {
                auto* player_window = new PlayerWindow(
                    sly::systems().at(entry->system_index), entry->path, entry->title, this
                );
                player_window->setAttribute(Qt::WA_DeleteOnClose);
                player_window->show();
            } catch (const std::exception& error) {
                QMessageBox::critical(
                    this, tr("Unable to Load ROM"), QString::fromUtf8(error.what())
                );
            }
        }
    );

    model->set_entries(
        RomListModel::entries_from_directory(QDir::current().filePath(QStringLiteral("roms")))
    );
}

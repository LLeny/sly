#include "main_window.h"

#include "rom_list_model.h"

#include "sly/sly.hpp"

#include <QDir>
#include <QHeaderView>
#include <QMenuBar>
#include <QSortFilterProxyModel>
#include <QStyledItemDelegate>
#include <QTableView>

#include <print>

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
    setWindowTitle(tr("Sly Desktop"));
    setMinimumSize(900, 500);
    resize(1180, 720);

    QMenuBar* menu_bar = new QMenuBar(this);
    setMenuBar(menu_bar);

    auto* file_menu = menu_bar->addMenu(tr("&File"));
    auto* cores_menu = menu_bar->addMenu(tr("&Cores"));

    file_menu->addAction(tr("&Load ROM"), this, [] {
        std::println("load rom stub");
    });
    file_menu->addAction(tr("&Quit"), QKeySequence::Quit, this, &QWidget::close);

    for (const auto system : sly::systems()) {
        const auto core_name = QString::fromStdString(system.name());

        cores_menu->addAction(core_name, this, [core_name]() {
            std::println("load core stub: {}", core_name.toStdString());
        });
    }

    auto* model = new RomListModel(this);
    auto* proxy = new QSortFilterProxyModel(this);
    proxy->setSourceModel(model);
    proxy->setSortCaseSensitivity(Qt::CaseInsensitive);
    proxy->setSortRole(RomListModel::SORT_ROLE);

    auto* table = new QTableView(this);
    table->setModel(proxy);
    configure_table(*table);
    setCentralWidget(table);

    model->set_entries(
        RomListModel::entries_from_directory(QDir::current().filePath(QStringLiteral("roms")))
    );
}

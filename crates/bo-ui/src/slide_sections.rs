//! Home > Slides as the reference presentation app lays it out at 1500 px (N6, round 5):
//! New Slide large, Layout, Reset and Section small; Section's menu (Add, Rename, Remove,
//! Remove All, Collapse All, Expand All) over the engine's sections ([MS-PPTX]
//! `p14:sectionLst`, `ViewCommand`); a click on a section header in the thumbnails pane
//! collapses or expands it, a double click renames it.
use super::ins::SlideDialog;
use super::SlideWindow;
use crate::dialog::{Dialog, DialogButton, DialogKind};
use crate::popup::{Choice, Menu, MenuItem};
use crate::ribbon::{Group, Item, RibbonHit};
use bo_engine::slide_edit::master::MasterCommand;
use bo_engine::slide_edit::views::ViewCommand;
use bo_render::Rgba;

/// Commands of the group added in this round.
pub const COMMANDS: [&str; 2] = ["slide.reset", "slide.section"];

/// Home > Slides.
pub(super) fn slides_group() -> Group {
    use Item::{Labeled as S, Large as L};
    Group {
        label: "slideEditor.groupSlides",
        large: vec![L("slide.new")],
        rows: vec![
            vec![S("slide.layout")],
            vec![S("slide.reset")],
            vec![S("slide.section")],
        ],
    }
}

pub(super) fn install_strings(s: &mut crate::strings::Strings) {
    for (id, key) in [("slide.reset", "resetSlide"), ("slide.section", "section")] {
        let v = s.get(&format!("slideNative.{key}")).to_owned();
        s.set(&format!("commands.{id}"), &v);
    }
}

pub(super) fn install_icons(icons: &mut crate::icons::Icons) {
    let blue = Rgba(0x44, 0x80, 0xc0, 255);
    // Reset: a slide with a turning arrow; Section: two slide stacks under a band (own
    // drawings).
    icons.add_solid(
        "slide.reset",
        &[
            ("M3 5h14v11h-14Z", Some(Rgba(255, 255, 255, 255))),
            ("M3 5h14v11h-14Z", None),
            ("M12 21a6 6 0 1 0-1-11", None),
            ("M8 9l3 1.5l-2.5 2.5Z", Some(blue)),
        ],
    );
    icons.add_solid(
        "slide.section",
        &[
            ("M2 3h20v4h-20Z", Some(Rgba(0xf0, 0xb4, 0x1e, 255))),
            (
                "M4 10h7v5h-7ZM13 10h7v5h-7ZM4 17h7v5h-7ZM13 17h7v5h-7Z",
                Some(Rgba(255, 255, 255, 255)),
            ),
            (
                "M4 10h7v5h-7ZM13 10h7v5h-7ZM4 17h7v5h-7ZM13 17h7v5h-7Z",
                None,
            ),
        ],
    );
}

impl SlideWindow {
    /// Run a command of the group; `None` when `id` is not one.
    pub(super) fn sections_action(&mut self, id: &str) -> Option<bool> {
        Some(match id {
            "slide.reset" => {
                let slides = self.view.selected_slides();
                let layout = self
                    .view
                    .engine()
                    .presentation()
                    .and_then(|p| p.slides.get(self.view.current()))
                    .map(|s| s.layout);
                match layout {
                    Some(layout) => self.view.run_master(MasterCommand::ApplyLayout {
                        layout,
                        slides: Some(slides),
                    }),
                    None => false,
                }
            }
            "slide.section" => {
                self.open_section_menu();
                true
            }
            _ => return None,
        })
    }

    fn open_section_menu(&mut self) {
        let Some(anchor) = self.ribbon.rect_of(RibbonHit::Command("slide.section")) else {
            return;
        };
        let any = !self.view.sections().is_empty();
        let item = |w: &Self, key: &str, c: &str, enabled: bool| MenuItem {
            label: w.t(&format!("slideNative.{key}"), key),
            choice: Choice::Command(format!("sect:{c}")),
            checked: false,
            dim: !enabled,
        };
        let items = vec![
            item(self, "addSection", "add", true),
            item(self, "renameSection", "rename", any),
            item(self, "removeSection", "remove", any),
            item(self, "removeAllSections", "removeAll", any),
            item(self, "collapseAll", "collapse", any),
            item(self, "expandAll", "expand", any),
        ];
        self.menu_kind = super::MenuKind::Duration;
        self.menu = Some(Menu::open(items, anchor, self.size, self.scale));
    }

    /// Menu choices (`sect:…`).
    pub(super) fn choose_section(&mut self, v: &str) -> bool {
        let cur = self.view.current_section();
        match v {
            "add" => {
                self.open_section_dialog(None);
                true
            }
            "rename" => {
                if let Some(i) = cur {
                    self.open_section_dialog(Some(i));
                }
                cur.is_some()
            }
            "remove" => cur.is_some_and(|index| {
                self.view.run_view(ViewCommand::RemoveSection {
                    index,
                    slides: false,
                })
            }),
            "removeAll" => self.view.run_view(ViewCommand::RemoveAllSections),
            "collapse" => self.view.collapse_all(true),
            "expand" => self.view.collapse_all(false),
            _ => false,
        }
    }

    /// The name dialog of Add Section (`None`) or Rename Section.
    pub(super) fn open_section_dialog(&mut self, rename: Option<usize>) {
        let name = rename
            .and_then(|i| self.view.sections().get(i).map(|s| s.name.clone()))
            .unwrap_or_else(|| self.t("slideNative.untitledSection", "Untitled Section"));
        let d = Dialog::new(
            DialogKind::Comment,
            self.t(
                if rename.is_some() {
                    "slideNative.renameSection"
                } else {
                    "slideNative.addSection"
                },
                "Section",
            ),
            vec![(self.t("slideNative.sectionName", "Section name:"), name)],
            vec![
                (self.t("slideNative.ok", "OK"), DialogButton::Ok),
                (self.t("slideNative.cancel", "Cancel"), DialogButton::Cancel),
            ],
            Vec::new(),
        );
        self.menu = None;
        self.picker = None;
        self.dialog = Some((d, SlideDialog::Section(rename)));
        self.dialog_layout = true;
    }

    /// The section dialog's button.
    pub(super) fn section_button(
        &mut self,
        rename: Option<usize>,
        b: DialogButton,
        values: &[String],
    ) {
        self.dialog = None;
        if b != DialogButton::Ok {
            return;
        }
        let name = values
            .first()
            .map(|v| v.trim().to_owned())
            .unwrap_or_default();
        if name.is_empty() {
            return;
        }
        if let Some(index) = rename {
            self.view
                .run_view(ViewCommand::RenameSection { index, name });
        } else {
            let first = self.t("slideNative.defaultSection", "Default Section");
            self.view.run_view(ViewCommand::AddSection {
                name,
                slide: Some(self.view.current()),
                first_name: Some(first),
            });
        }
    }

    /// A press on a section header: collapse / expand, or rename on a double click.
    pub(super) fn section_press(&mut self, x: f32, y: f32, count: u8) -> Option<bool> {
        let i = self.view.section_at(x, y)?;
        if count >= 2 {
            self.open_section_dialog(Some(i));
            return Some(true);
        }
        Some(self.view.toggle_section(i))
    }
}

use crate::event::EventRequest;

use egui::Widget;

/// A popup that has a specified action to run on confirmation.
pub struct ActionPopup {
    /// The action to run on confirmation
    action: EventRequest,
    /// The popup definition
    popup: Popup,
}

impl ActionPopup {
    pub fn builder(action: EventRequest) -> ActionPopupBuilder {
        ActionPopupBuilder {
            action,
            builder: PopupBuilder::default(),
        }
    }

    /// Wether the user has made a choice
    pub fn is_finished(&self) -> bool {
        self.popup.finished
    }

    /// The choice of the user
    pub fn is_confirmed(&self) -> bool {
        self.popup.confirmed
    }

    /// Consume this popup returning the action that should be run
    pub fn action(self) -> EventRequest {
        self.action
    }
}

impl Widget for &mut ActionPopup {
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        self.popup.ui(ui)
    }
}

#[derive(Debug)]
pub struct Popup {
    title: String,
    text: String,
    confirmed: bool,
    window_enabled: bool,
    finished: bool,
}

impl Popup {
    pub fn builder() -> PopupBuilder {
        PopupBuilder::default()
    }
}

impl Widget for &mut Popup {
    fn ui(mut self, ui: &mut egui::Ui) -> egui::Response {
        let mut confirmed = self.confirmed;
        let mut enabled = true;
        if !self.finished {
            let response = egui::Window::new(&self.title)
                .id(egui::Id::new(&self.text))
                .enabled(enabled)
                .collapsible(false)
                .show(ui.ctx(), |ui| {
                    egui::Grid::new(&self.title)
                        .show(ui, |ui| {
                            ui.label(&self.text);
                            ui.end_row();
                            ui.scope(|ui| {
                                if ui.button("yes").clicked() {
                                    confirmed = true;
                                    enabled = false;
                                }
                                if ui.button("no").clicked() {
                                    confirmed = false;
                                    enabled = false;
                                }
                            });
                        })
                        .response
                })
                .map(|r| r.response);
            if !enabled {
                self.finished = true;
            }
            self.confirmed = confirmed;
            self.window_enabled = enabled;
            if let Some(rsp) = response {
                return rsp;
            }
        }
        ui.scope(|_| {}).response
    }
}

#[derive(Debug)]
pub struct ActionPopupBuilder {
    action: EventRequest,
    builder: PopupBuilder,
}

impl ActionPopupBuilder {
    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.builder = self.builder.title(title);
        self
    }

    pub fn text(mut self, text: impl Into<String>) -> Self {
        self.builder = self.builder.text(text);
        self
    }

    pub fn build(self) -> ActionPopup {
        ActionPopup {
            action: self.action,
            popup: self.builder.build(),
        }
    }
}

#[derive(Debug, Default)]
pub struct PopupBuilder {
    title: String,
    text: String,
}

impl PopupBuilder {
    pub fn build(self) -> Popup {
        Popup {
            title: self.title,
            text: self.text,
            confirmed: false,
            finished: false,
            window_enabled: false,
        }
    }

    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.title = title.into();
        self
    }

    pub fn text(mut self, text: impl Into<String>) -> Self {
        self.text = text.into();
        self
    }
}

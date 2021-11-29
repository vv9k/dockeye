use crate::event::EventRequest;

/// A popup that has a specified action to run on confirmation.
pub struct ActionPopup {
    /// The action to run on confirmation
    action: EventRequest,
    /// The popup definition
    popup: Popup,
}

impl ActionPopup {
    /// Creates a new action popup
    pub fn new(action: EventRequest, title: impl Into<String>, text: impl Into<String>) -> Self {
        Self {
            action,
            popup: Popup::new(title.into(), text.into()),
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

    /// Display this popup
    pub fn display(&mut self, ctx: &egui::CtxRef) -> Option<egui::Response> {
        self.popup.display(ctx)
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
    pub fn new(title: String, text: String) -> Self {
        Self {
            title,
            text,
            confirmed: false,
            window_enabled: false,
            finished: false,
        }
    }

    pub fn display(&mut self, ctx: &egui::CtxRef) -> Option<egui::Response> {
        let mut confirmed = self.confirmed;
        let mut enabled = true;
        if !self.finished {
            let response = popup(ctx, &self.title, &self.text, &mut confirmed, &mut enabled);
            if !enabled {
                self.finished = true;
            }
            self.confirmed = confirmed;
            self.window_enabled = enabled;
            return response;
        }
        None
    }
}

pub fn popup(
    ctx: &egui::CtxRef,
    title: &str,
    text: &str,
    confirmed: &mut bool,
    enabled: &mut bool,
) -> Option<egui::Response> {
    egui::Window::new(title)
        .id(egui::Id::new(text))
        .enabled(*enabled)
        .collapsible(false)
        .show(ctx, |ui| {
            egui::Grid::new(title)
                .show(ui, |ui| {
                    ui.label(text);
                    ui.end_row();
                    ui.scope(|ui| {
                        if ui.button("yes").clicked() {
                            *confirmed = true;
                            *enabled = false;
                        }
                        if ui.button("no").clicked() {
                            *confirmed = false;
                            *enabled = false;
                        }
                    });
                })
                .response
        })
        .map(|r| r.response)
}

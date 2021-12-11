use crate::app::ui::{icon, key};

#[derive(Debug)]
enum ListData<'a> {
    KeyVal(&'a mut Vec<(String, String)>),
    Key(&'a mut Vec<String>),
}

#[derive(Debug)]
pub struct EditableList<'a> {
    heading: Option<&'a str>,
    data: ListData<'a>,
    id_source: Option<&'a str>,
    desired_width: Option<f32>,
    key_heading: Option<&'a str>,
    val_heading: Option<&'a str>,
    add_hover_text: Option<&'a str>,
}

impl<'a> EditableList<'a> {
    pub fn builder_key_val(data: &'a mut Vec<(String, String)>) -> EditableListBuilder<'a> {
        EditableListBuilder {
            heading: None,
            data: ListData::KeyVal(data),
            id_source: None,
            desired_width: None,
            key_heading: None,
            val_heading: None,
            add_hover_text: None,
        }
    }

    pub fn builder_key(data: &'a mut Vec<String>) -> EditableListBuilder<'a> {
        EditableListBuilder {
            heading: None,
            data: ListData::Key(data),
            id_source: None,
            desired_width: None,
            key_heading: None,
            val_heading: None,
            add_hover_text: None,
        }
    }

    pub fn show(&mut self, ui: &mut egui::Ui) -> egui::Response {
        if let Some(heading) = self.heading {
            key!(ui, heading);
        }
        if ui
            .button(icon::ADD)
            .on_hover_text(self.add_hover_text.unwrap_or_default())
            .clicked()
        {
            match &mut self.data {
                ListData::Key(data) => data.push(Default::default()),
                ListData::KeyVal(data) => data.push(Default::default()),
            }
        }
        ui.end_row();
        ui.scope(|_| {});
        let mut to_delete = None;
        let desired_width = self.desired_width.unwrap_or(200.);
        let desired_size = egui::vec2(desired_width, 0.);
        let grid = if let Some(id) = self.id_source {
            egui::Grid::new(id)
        } else {
            egui::Grid::new(self.heading.unwrap_or_default())
        };

        let response = grid.show(ui, |ui| match &mut self.data {
            ListData::Key(data) => {
                for (i, key) in data.iter_mut().enumerate() {
                    ui.add(egui::TextEdit::singleline(key));
                    if ui.button(icon::DELETE).clicked() {
                        to_delete = Some(i);
                    }
                    ui.end_row();
                }
                ui.scope(|_| {});
                ui.allocate_space(desired_size);
                ui.end_row();
            }
            ListData::KeyVal(data) => {
                for (i, (key, val)) in data.iter_mut().enumerate() {
                    if let Some(key_heading) = self.key_heading {
                        key!(ui, key_heading);
                    }
                    ui.add(egui::TextEdit::singleline(key));
                    if let Some(val_heading) = self.val_heading {
                        key!(ui, val_heading);
                    }
                    ui.add(egui::TextEdit::singleline(val));
                    if ui.button(icon::DELETE).clicked() {
                        to_delete = Some(i);
                    }
                    ui.end_row();
                }
                ui.scope(|_| {});
                ui.allocate_space(desired_size);
                ui.scope(|_| {});
                ui.allocate_space(desired_size);
                ui.end_row();
            }
        });
        if let Some(idx) = to_delete {
            match &mut self.data {
                ListData::Key(data) => {
                    data.remove(idx);
                }
                ListData::KeyVal(data) => {
                    data.remove(idx);
                }
            };
        }
        response.response
    }
}

#[derive(Debug)]
pub struct EditableListBuilder<'a> {
    heading: Option<&'a str>,
    data: ListData<'a>,
    id_source: Option<&'a str>,
    desired_width: Option<f32>,
    key_heading: Option<&'a str>,
    val_heading: Option<&'a str>,
    add_hover_text: Option<&'a str>,
}

impl<'a> EditableListBuilder<'a> {
    pub fn build(self) -> EditableList<'a> {
        EditableList {
            heading: self.heading,
            data: self.data,
            id_source: self.id_source,
            desired_width: self.desired_width,
            key_heading: self.key_heading,
            val_heading: self.val_heading,
            add_hover_text: self.add_hover_text,
        }
    }

    pub fn heading(mut self, heading: &'a str) -> Self {
        self.heading = Some(heading);
        self
    }

    #[allow(dead_code)]
    pub fn id_source(mut self, id_source: &'a str) -> Self {
        self.id_source = Some(id_source);
        self
    }

    pub fn key_heading(mut self, key_heading: &'a str) -> Self {
        self.key_heading = Some(key_heading);
        self
    }

    pub fn val_heading(mut self, val_heading: &'a str) -> Self {
        self.val_heading = Some(val_heading);
        self
    }

    pub fn add_hover_text(mut self, add_hover_text: &'a str) -> Self {
        self.add_hover_text = Some(add_hover_text);
        self
    }
}

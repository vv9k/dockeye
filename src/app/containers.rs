use crate::app::{
    key, key_val, val, App, DELETE_ICON, INFO_ICON, PACKAGE_ICON, PAUSE_ICON, PLAY_ICON, STOP_ICON,
};
use crate::event::EventRequest;

use docker_api::api::{ContainerDetails, ContainerStatus};
use egui::widgets::plot::{self, Line, Plot};
use egui::{Grid, Label};

pub fn is_running(container: &ContainerDetails) -> bool {
    matches!(container.state.status, ContainerStatus::Running)
}

pub fn is_paused(container: &ContainerDetails) -> bool {
    matches!(container.state.status, ContainerStatus::Paused)
}
macro_rules! btn {
    ($self:ident, $ui:ident, $icon:ident, $hover:literal, $event:expr, $errors: ident) => {
        if $ui.button($icon).on_hover_text($hover).clicked() {
            if let Err(e) = $self.send_event($event) {
                $errors.push(Box::new(e));
            }
        }
    };
    (stop => $self:ident, $ui:ident, $container:ident, $errors:ident) => {
        btn!(
            $self,
            $ui,
            STOP_ICON,
            "stop the container",
            EventRequest::StopContainer {
                id: $container.id.clone()
            },
            $errors
        );
    };
    (start => $self:ident, $ui:ident, $container:ident, $errors:ident) => {
        btn!(
            $self,
            $ui,
            PLAY_ICON,
            "start the container",
            EventRequest::StartContainer {
                id: $container.id.clone()
            },
            $errors
        );
    };
    (pause => $self:ident, $ui:ident, $container:ident, $errors:ident) => {
        btn!(
            $self,
            $ui,
            PAUSE_ICON,
            "pause the container",
            EventRequest::PauseContainer {
                id: $container.id.clone()
            },
            $errors
        );
    };
    (unpause => $self:ident, $ui:ident, $container:ident, $errors:ident) => {
        btn!(
            $self,
            $ui,
            PLAY_ICON,
            "unpause the container",
            EventRequest::UnpauseContainer {
                id: $container.id.clone()
            },
            $errors
        );
    };
    (info => $self:ident, $ui:ident, $container:ident, $errors:ident) => {
        btn!(
            $self,
            $ui,
            INFO_ICON,
            "inpect the container",
            EventRequest::ContainerTraceStart {
                id: $container.id.clone()
            },
            $errors
        );
    };
    (delete => $self:ident, $ui:ident, $container:ident, $errors:ident) => {
        btn!(
            $self,
            $ui,
            DELETE_ICON,
            "delete the container",
            EventRequest::DeleteContainer {
                id: $container.id.clone()
            },
            $errors
        );
    };
}

impl App {
    pub fn containers_scroll(&mut self, ui: &mut egui::Ui) {
        egui::ScrollArea::vertical().show(ui, |ui| {
            egui::Grid::new("side_panel")
                .min_col_width(100.)
                .max_col_width(self.side_panel_size())
                .show(ui, |ui| {
                    let mut errors = vec![];
                    for container in &self.containers {
                        let color = if &container.state == "running" {
                            egui::Color32::GREEN
                        } else if &container.state == "paused" {
                            egui::Color32::YELLOW
                        } else {
                            egui::Color32::RED
                        };
                        let dot = egui::Label::new(PACKAGE_ICON).text_color(color).heading();
                        ui.scope(|ui| {
                            egui::Grid::new(&container.id)
                                .min_col_width(100.)
                                .max_col_width(self.side_panel_size())
                                .show(ui, |ui| {
                                    ui.scope(|ui| {
                                        ui.add(dot);
                                        if let Some(name) = container.names.first() {
                                            ui.add(
                                                Label::new(name.trim_start_matches('/')).strong(),
                                            );
                                        } else {
                                            ui.add(Label::new(&container.id[..12]).strong());
                                        }
                                    });
                                    let image = if container.image.starts_with("sha256") {
                                        &container.image.trim_start_matches("sha256:")[..12]
                                    } else {
                                        container.image.as_str()
                                    };
                                    ui.end_row();
                                    ui.add(Label::new(image).italics());
                                    ui.end_row();

                                    ui.add(Label::new(&container.status).italics());
                                    ui.end_row();

                                    ui.scope(|ui| {
                                        btn!(info => self, ui, container, errors);
                                        btn!(delete => self, ui, container, errors);
                                        if &container.state == "running" {
                                            btn!(stop => self, ui, container, errors);
                                            btn!(pause => self, ui, container, errors);
                                        } else if &container.state == "paused" {
                                            btn!(stop => self, ui, container, errors);
                                            btn!(unpause => self, ui, container, errors);
                                        } else {
                                            btn!(start => self, ui, container, errors);
                                        }
                                    });
                                    ui.end_row();
                                });
                        });
                        ui.end_row();

                        ui.separator();
                        ui.end_row();
                    }
                    errors.into_iter().for_each(|error| self.add_error(error));
                });
        });
    }

    pub fn container_details(&mut self, ui: &mut egui::Ui) {
        let mut errors = vec![];
        if let Some(container) = &self.current_container {
            let color = if is_running(container) {
                egui::Color32::GREEN
            } else if is_paused(container) {
                egui::Color32::YELLOW
            } else {
                egui::Color32::RED
            };
            ui.horizontal(|ui| {
                ui.add(egui::Label::new(PACKAGE_ICON).text_color(color).heading());
                ui.add(
                    Label::new(container.name.trim_start_matches('/'))
                        .heading()
                        .wrap(true)
                        .strong(),
                );
            });
            ui.add_space(25.);
            self.container_buttons(ui, container, &mut errors);
            self.container_info(ui, container);
            self.container_stats(ui);
            self.container_logs(ui);
        }
        errors.into_iter().for_each(|error| self.add_error(error));
    }

    fn container_buttons(
        &self,
        ui: &mut egui::Ui,
        container: &ContainerDetails,
        errors: &mut Vec<Box<dyn std::fmt::Display>>,
    ) {
        if is_running(container) {
            ui.horizontal(|ui| {
                btn!(stop => self, ui, container, errors);
                btn!(pause => self, ui, container, errors);
            });
        } else if is_paused(container) {
            ui.horizontal(|ui| {
                btn!(stop => self, ui, container, errors);
                btn!(unpause => self, ui, container, errors);
            });
        } else {
            ui.horizontal(|ui| {
                btn!(start => self, ui, container, errors);
            });
        }
    }

    fn container_info(&self, ui: &mut egui::Ui, container: &ContainerDetails) {
        Grid::new("container_info").show(ui, |ui| {
            key_val!(ui, "ID:", &container.id);
            key_val!(ui, "Image:", &container.image);
            key_val!(
                ui,
                "Command:",
                &container.config.cmd.as_deref().unwrap_or(&[]).join(" ")
            );

            if let Some(entrypoint) = container.config.entrypoint.as_ref() {
                key_val!(ui, "Entrypoint:", &entrypoint.join(" "));
            }

            key!(ui, "Labels:");
            ui.end_row();
            if let Some(labels) = container.config.labels.as_ref() {
                if !labels.is_empty() {
                    ui.label("          ");
                    Grid::new("labels_grid").show(ui, |ui| {
                        let mut labels = labels.iter().collect::<Vec<_>>();
                        labels.sort();
                        for (k, v) in labels {
                            val!(ui, &k);
                            val!(ui, &v);
                            ui.end_row();
                        }
                    });
                    ui.end_row();
                }
            }

            key_val!(ui, "Created:", container.created.to_rfc2822());
            key_val!(ui, "State:", container.state.status.as_ref());
            key_val!(ui, "Hostname:", &container.config.hostname);

            if !container.config.domainname.is_empty() {
                key_val!(ui, "Domainname:", &container.config.domainname);
            }

            key_val!(ui, "User:", &container.config.user);
            key_val!(ui, "Working dir:", &container.config.working_dir);

            if let Some(shell) = container.config.shell.as_ref() {
                key_val!(ui, "Shell:", &shell.join(" "));
            }

            key!(ui, "Env: ");
            ui.end_row();
            if !container.config.env.is_empty() {
                for var in &container.config.env {
                    ui.scope(|_| {});
                    val!(ui, &var);
                    ui.end_row();
                }
            }

            if let Some(size_rw) = &container.size_rw {
                key_val!(ui, "Size RW:", &format!("{}", size_rw));
            }

            if let Some(size_root_fs) = &container.size_root_fs {
                key_val!(ui, "Size rootfs:", &format!("{}", size_root_fs));
            }

            key_val!(ui, "Restart count:", container.restart_count);

            key_val!(ui, "Driver:", &container.driver);
            key_val!(ui, "Platform:", &container.platform);

            key!(ui, "Networks:");
            if !container.network_settings.networks.is_empty() {
                egui::CollapsingHeader::new("").show(ui, |ui| {
                    egui::Grid::new("networks_grid").show(ui, |ui| {
                        for (name, entry) in &container.network_settings.networks {
                            key!(ui, name.as_str());
                            ui.end_row();
                            ui.scope(|_| {});
                            egui::Grid::new(&name).show(ui, |ui| {
                                key_val!(ui, "MAC address:", &entry.mac_address);
                                key_val!(
                                    ui,
                                    "IPv4:",
                                    format!("{}/{}", entry.ip_address, entry.ip_prefix_len)
                                );
                                key_val!(ui, "Gateway:", &entry.gateway);
                                key_val!(
                                    ui,
                                    "IPv6:",
                                    format!(
                                        "{}/{}",
                                        entry.global_ipv6_address, entry.global_ipv6_prefix_len
                                    )
                                );
                                key_val!(ui, "IPv6 gateway:", &entry.ipv6_gateway);
                                key_val!(ui, "Network ID:", &entry.network_id);
                                key_val!(ui, "Endpoint ID:", &entry.endpoint_id);
                            });
                            ui.end_row();
                            ui.scope(|_| {});
                            ui.separator();
                            ui.end_row();
                        }
                    });
                });
                ui.end_row();
            }
        });
    }

    fn container_stats(&self, ui: &mut egui::Ui) {
        if let Some(stats) = &self.current_stats {
            egui::CollapsingHeader::new("Stats")
                .default_open(false)
                .show(ui, |ui| {
                    let cpu_data =
                        plot::Values::from_values_iter(stats.0.iter().map(|(time, stat)| {
                            plot::Value::new(time.as_secs_f64(), stat.cpu_usage)
                        }));

                    let mem_data =
                        plot::Values::from_values_iter(stats.0.iter().map(|(time, stat)| {
                            plot::Value::new(time.as_secs_f64(), stat.mem_percent)
                        }));

                    Grid::new("stats_grid").show(ui, |ui| {
                        if let Some(last) = stats.0.last() {
                            key_val!(ui, "CPU usage:", format!("{:0.2}%", last.1.cpu_usage));

                            key_val!(
                                ui,
                                "Memory usage:",
                                format!(
                                    "{} / {}  {:0.2}%",
                                    crate::conv_fb(last.1.mem_usage),
                                    crate::conv_fb(last.1.mem_limit),
                                    last.1.mem_percent
                                )
                            );

                            if let Some(net_stat) = &last.1.net_stat {
                                let (rx, tx) =
                                    net_stat.into_iter().fold((0, 0), |mut acc, (_, stats)| {
                                        acc.0 += stats.rx_bytes;
                                        acc.1 += stats.tx_bytes;
                                        acc
                                    });
                                key_val!(
                                    ui,
                                    "Network I/O:",
                                    format!("{} / {}", crate::conv_b(rx), crate::conv_b(tx))
                                );
                            }

                            if let Some(mem_stat) = &last.1.mem_stat {
                                ui.add(Label::new("Memory stats:").strong());
                                egui::CollapsingHeader::new("")
                                    .id_source("mem_stats")
                                    .default_open(false)
                                    .show(ui, |ui| {
                                        Grid::new("mem_stats_grid").show(ui, |ui| {
                                            macro_rules! mem_key_val {
                                                ($k:literal, $v:expr) => {
                                                    if let Some(v) = $v {
                                                        key_val!(ui, $k, v.to_string());
                                                    }
                                                };
                                            }
                                            mem_key_val!("Cache:", mem_stat.cache);
                                            mem_key_val!("Total cache:", mem_stat.total_cache);
                                            mem_key_val!("Active files:", mem_stat.active_file);
                                            mem_key_val!(
                                                "Total active files:",
                                                mem_stat.total_active_file
                                            );
                                            mem_key_val!("Inactive files:", mem_stat.inactive_file);
                                            mem_key_val!(
                                                "Total inactive files:",
                                                mem_stat.total_inactive_file
                                            );
                                            mem_key_val!("Mapped files:", mem_stat.mapped_file);
                                            mem_key_val!(
                                                "Total mapped files:",
                                                mem_stat.total_mapped_file
                                            );
                                            mem_key_val!("Page out:", mem_stat.pgpgout);
                                            mem_key_val!("Total page out:", mem_stat.total_pgpgout);
                                            mem_key_val!("Page in:", mem_stat.pgpgin);
                                            mem_key_val!("Total page in:", mem_stat.total_pgpgin);
                                            mem_key_val!("Page faults:", mem_stat.pgfault);
                                            mem_key_val!(
                                                "Total page faults:",
                                                mem_stat.total_pgfault
                                            );
                                            mem_key_val!("Page major faults:", mem_stat.pgmajfault);
                                            mem_key_val!(
                                                "Total page major faults:",
                                                mem_stat.total_pgmajfault
                                            );
                                            mem_key_val!("Active anonymous:", mem_stat.active_anon);
                                            mem_key_val!(
                                                "Total active anonymous:",
                                                mem_stat.total_active_anon
                                            );
                                            mem_key_val!(
                                                "Inactive anonymous:",
                                                mem_stat.inactive_anon
                                            );
                                            mem_key_val!(
                                                "Total inactive anonymous:",
                                                mem_stat.total_active_anon
                                            );
                                            mem_key_val!("RSS:", mem_stat.rss);
                                            mem_key_val!("Total RSS:", mem_stat.total_rss);
                                            mem_key_val!("RSS huge:", mem_stat.rss_huge);
                                            mem_key_val!(
                                                "Total RSS huge:",
                                                mem_stat.total_rss_huge
                                            );
                                            mem_key_val!("Unevictable:", mem_stat.unevictable);
                                            mem_key_val!(
                                                "Total unevictable:",
                                                mem_stat.total_unevictable
                                            );
                                            mem_key_val!("Writeback:", mem_stat.writeback);
                                            mem_key_val!(
                                                "Total writeback:",
                                                mem_stat.total_writeback
                                            );
                                            mem_key_val!(
                                                "Hierarchical memory limit:",
                                                mem_stat.hierarchical_memory_limit
                                            );
                                            mem_key_val!(
                                                "Hierarchical memsw limit:",
                                                mem_stat.hierarchical_memsw_limit
                                            );
                                        });
                                    });
                            }
                            ui.end_row();

                            if let Some(net_stat) = &last.1.net_stat {
                                ui.add(Label::new("Network stats:").strong());
                                egui::CollapsingHeader::new("")
                                    .id_source("net_stats")
                                    .default_open(false)
                                    .show(ui, |ui| {
                                        for (network, stats) in net_stat {
                                            egui::CollapsingHeader::new(&network)
                                                .default_open(false)
                                                .show(ui, |ui| {
                                                    Grid::new(&network).show(ui, |ui| {
                                                        key_val!(ui, "rx_bytes", stats.rx_bytes);
                                                        key_val!(ui, "tx_bytes", stats.tx_bytes);
                                                        key_val!(
                                                            ui,
                                                            "rx_packets",
                                                            stats.rx_packets
                                                        );
                                                        key_val!(
                                                            ui,
                                                            "tx_packets",
                                                            stats.tx_packets
                                                        );
                                                        key_val!(
                                                            ui,
                                                            "rx_dropped",
                                                            stats.rx_dropped
                                                        );
                                                        key_val!(
                                                            ui,
                                                            "tx_dropped",
                                                            stats.tx_dropped
                                                        );
                                                        key_val!(ui, "rx_errors", stats.rx_errors);
                                                        key_val!(ui, "tx_errors", stats.tx_errors);
                                                    });
                                                });
                                        }
                                    });
                            }
                        }
                    });
                    ui.add(
                        Plot::new("CPU usage")
                            .data_aspect(1.5)
                            .height(self.graph_height())
                            .include_x(0.)
                            .include_y(0.)
                            .legend(egui::widgets::plot::Legend {
                                position: egui::widgets::plot::Corner::RightTop,
                                ..Default::default()
                            })
                            .line(
                                Line::new(cpu_data)
                                    .name("CPU usage %")
                                    .color(egui::Color32::YELLOW),
                            ),
                    );
                    ui.add(
                        Plot::new("Memory usage")
                            .data_aspect(1.5)
                            .height(self.graph_height())
                            .include_x(0.)
                            .include_y(0.)
                            .legend(egui::widgets::plot::Legend {
                                position: egui::widgets::plot::Corner::RightTop,
                                ..Default::default()
                            })
                            .line(
                                Line::new(mem_data)
                                    .name("Memory usage %")
                                    .color(egui::Color32::BLUE),
                            ),
                    );
                });
        }
    }

    fn container_logs(&mut self, ui: &mut egui::Ui) {
        if let Some(logs) = &self.current_logs {
            egui::CollapsingHeader::new("Logs")
                .default_open(false)
                .show(ui, |ui| {
                    //let mut layouter = |ui: &egui::Ui, string: &str, wrap_width: f32| {
                    //let mut layout_job: egui::text::LayoutJob = my_memoized_highlighter(string);
                    //layout_job.wrap_width = wrap_width;
                    //ui.fonts().layout_job(layout_job)
                    //};

                    ui.add(egui::TextEdit::multiline(&mut logs.as_str()).code_editor());
                    //for (i, line) in lines.enumerate().map(|(i, line)| (i + 1, line)) {
                    //ui.horizontal(|ui| {
                    //let i_count = crate::checked_log_10(i).unwrap_or(1) + 1;
                    //ui.add(
                    //Label::new(format!(
                    //"{}{}|",
                    //" ".repeat((max_count - i_count) as usize),
                    //i + self.logs_page * MAX_LINES
                    //))
                    //.code()
                    //.strong(),
                    //);
                    //ui.add(Label::new(line).monospace());
                    //});
                    //}

                    //ui.add_space(10.);
                    //ui.label(format!("Page: {} / {}", self.logs_page + 1, total_pages));

                    //ui.horizontal(|ui| {
                    //if ui.button("previous page").clicked() && self.logs_page > 0 {
                    //self.logs_page -= 1;
                    //}

                    //if ui.button("next page").clicked() && (self.logs_page + 1) < total_pages {
                    //self.logs_page += 1;
                    //}
                    //});
                });
        }
    }
}

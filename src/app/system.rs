use crate::app::ui::{icon, key, key_val, val};
use crate::app::App;
use crate::app::{containers, images};
use crate::event::SystemInspectInfo;

use docker_api::api::{DataUsage, Event};
use egui::{CollapsingHeader, Grid};

const MAX_ITEM_COUNT: usize = 10;

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum CentralView {
    Home,
    Events,
    DataUsage,
}

impl Default for CentralView {
    fn default() -> Self {
        CentralView::Home
    }
}

#[derive(Default, Debug)]
pub struct SystemTab {
    pub system_info: Option<Box<SystemInspectInfo>>,
    pub data_usage: Option<Box<DataUsage>>,
    pub central_view: CentralView,
    pub display_all_containers: bool,
    pub display_all_images: bool,
    pub display_all_cache: bool,
    pub events: Vec<Event>,
}

impl App {
    pub fn system_view(&mut self, ui: &mut egui::Ui) {
        egui::ScrollArea::vertical().show(ui, |ui| match self.system.central_view {
            CentralView::Home => self.system_details(ui),
            CentralView::Events => self.system_events(ui),
            CentralView::DataUsage => self.system_data_usage(ui),
        });
    }

    pub fn system_side(&mut self, ui: &mut egui::Ui) {
        self.system_menu(ui);
    }

    fn system_menu(&mut self, ui: &mut egui::Ui) {
        Grid::new("system_menu").show(ui, |ui| {
            ui.selectable_value(&mut self.system.central_view, CentralView::Home, "home");
            ui.selectable_value(&mut self.system.central_view, CentralView::Events, "events");
            ui.selectable_value(
                &mut self.system.central_view,
                CentralView::DataUsage,
                "data usage",
            );
        });
    }

    fn system_events(&mut self, ui: &mut egui::Ui) {
        ui.add(egui::Label::new("System events").heading().strong());
        ui.add_space(25.);
        if !self.system.events.is_empty() {
            Grid::new("system_events_grid")
                .spacing((10., 10.))
                .striped(true)
                .show(ui, |ui| {
                    key!(ui, "Id");
                    key!(ui, "Type");
                    key!(ui, "Actor");
                    key!(ui, "Action");
                    key!(ui, "Time");
                    key!(ui, "From");
                    key!(ui, "Status");
                    ui.end_row();
                    for event in &self.system.events {
                        val!(ui, event.id.as_deref().unwrap_or_default());
                        val!(ui, &event.typ);
                        val!(ui, &event.actor.id);
                        val!(ui, &event.action);
                        val!(ui, &event.time.to_rfc2822());
                        val!(ui, event.from.as_deref().unwrap_or_default());
                        val!(ui, event.status.as_deref().unwrap_or_default());
                        ui.end_row();
                    }
                });
        }
    }

    fn system_data_usage(&mut self, ui: &mut egui::Ui) {
        ui.allocate_space((f32::INFINITY, 0.).into());
        self.containers_data_usage(ui);
        ui.add_space(10.);
        self.images_data_usage(ui);
        ui.add_space(10.);
        self.build_cache_usage(ui);
    }

    fn containers_data_usage(&mut self, ui: &mut egui::Ui) {
        CollapsingHeader::new("containers")
            .default_open(false)
            .show(ui, |ui| {
                if let Some(usage) = self.system.data_usage.as_ref() {
                    let (count, total_root, total_rw) =
                        usage.containers.iter().fold((0, 0, 0), |mut acc, c| {
                            acc.0 += 1;
                            acc.1 += c.size_root_fs.unwrap_or_default() as u64;
                            acc.2 += c.size_rw.unwrap_or_default() as u64;
                            acc
                        });
                    ui.checkbox(&mut self.system.display_all_containers, "Display all");
                    ui.add_space(10.);
                    Grid::new("total_size_grid").show(ui, |ui| {
                        key!(ui, "Total size:");
                        Grid::new("size_details_grid").show(ui, |ui| {
                            ui.label("Root FS");
                            ui.label("RW");
                            ui.end_row();
                            val!(ui, crate::conv_b(total_root));
                            val!(ui, crate::conv_b(total_rw));
                        });
                    });
                    Grid::new("containers_grid")
                        .spacing((20., 10.))
                        .min_col_width(50.)
                        .striped(true)
                        .show(ui, |ui| {
                            key!(ui, "ID");
                            key!(ui, "Created");
                            key!(ui, "Image");
                            key!(ui, "Command");
                            key!(ui, "Size Total");
                            key!(ui, "Size RW");
                            ui.end_row();
                            let mut containers = usage.containers.iter().collect::<Vec<_>>();
                            containers.sort_by(|a, b| b.size_root_fs.cmp(&a.size_root_fs));

                            fn container_stats(
                                ui: &mut egui::Ui,
                                container: &docker_api::api::ContainerSummary,
                            ) {
                                let name = if let Some(first) = container.names.first() {
                                    first.trim_start_matches('/')
                                } else {
                                    &container.id[0..12]
                                };
                                let color = containers::color_for_state(&container.state);
                                let icon = containers::state_icon(color);
                                ui.scope(|ui| {
                                    ui.add(icon);
                                    val!(ui, name);
                                });
                                let naive =
                                    chrono::NaiveDateTime::from_timestamp(container.created, 0);
                                let datetime: chrono::DateTime<chrono::Utc> =
                                    chrono::DateTime::from_utc(naive, chrono::Utc);
                                let command = if container.command.len() > 32 {
                                    &container.command[..32]
                                } else {
                                    &container.command[..]
                                };
                                val!(ui, datetime.to_rfc2822());
                                val!(ui, images::trim_id(&container.image));
                                val!(ui, command);
                                val!(
                                    ui,
                                    crate::conv_b(container.size_root_fs.unwrap_or_default() as u64)
                                );
                                val!(
                                    ui,
                                    crate::conv_b(container.size_rw.unwrap_or_default() as u64)
                                );
                            }

                            if self.system.display_all_containers {
                                for container in containers {
                                    container_stats(ui, container);
                                    ui.end_row();
                                }
                            } else {
                                for container in containers.iter().take(MAX_ITEM_COUNT) {
                                    container_stats(ui, container);
                                    ui.end_row();
                                }
                            }
                        });
                    if !self.system.display_all_containers && count > MAX_ITEM_COUNT {
                        ui.add(egui::Label::new("More to load...").weak());
                    }
                }
            });
    }

    fn images_data_usage(&mut self, ui: &mut egui::Ui) {
        CollapsingHeader::new("images")
            .default_open(false)
            .show(ui, |ui| {
                if let Some(usage) = self.system.data_usage.as_ref() {
                    let (count, total, total_shared, total_virtual) =
                        usage.images.iter().fold((0, 0, 0, 0), |mut acc, i| {
                            acc.0 += 1;
                            acc.1 += i.size;
                            acc.2 += i.shared_size;
                            acc.3 += i.virtual_size;
                            acc
                        });
                    ui.checkbox(&mut self.system.display_all_images, "Display all");
                    ui.add_space(10.);
                    Grid::new("total_images_size_grid").show(ui, |ui| {
                        key!(ui, "Total size:");
                        Grid::new("images_size_details_grid").show(ui, |ui| {
                            key!(ui, "Total");
                            key!(ui, "Shared");
                            key!(ui, "Virtual");
                            ui.end_row();
                            val!(ui, crate::conv_b(total as u64));
                            val!(ui, crate::conv_b(total_shared as u64));
                            val!(ui, crate::conv_b(total_virtual as u64));
                        });
                    });
                    Grid::new("images_grid")
                        .spacing((20., 10.))
                        .min_col_width(50.)
                        .striped(true)
                        .show(ui, |ui| {
                            key!(ui, "ID");
                            key!(ui, "Created");
                            key!(ui, "Containers");
                            key!(ui, "Size");
                            key!(ui, "Shared size");
                            key!(ui, "Virtual size");
                            ui.end_row();
                            let mut images = usage.images.iter().collect::<Vec<_>>();
                            images.sort_by(|a, b| b.size.cmp(&a.size));

                            fn image_stats(
                                ui: &mut egui::Ui,
                                image: &docker_api::api::ImageSummary,
                            ) {
                                let name = if let Some(first) =
                                    image.repo_tags.as_ref().and_then(|tags| tags.first())
                                {
                                    first
                                } else {
                                    images::trim_id(&image.id)
                                };
                                let naive =
                                    chrono::NaiveDateTime::from_timestamp(image.created as i64, 0);
                                let datetime: chrono::DateTime<chrono::Utc> =
                                    chrono::DateTime::from_utc(naive, chrono::Utc);

                                ui.scope(|ui| {
                                    ui.add(images::icon());
                                    val!(ui, name);
                                });
                                val!(ui, datetime.to_rfc2822());
                                val!(ui, image.containers);
                                val!(ui, crate::conv_b(image.size as u64));
                                val!(ui, crate::conv_b(image.shared_size as u64));
                                val!(ui, crate::conv_b(image.virtual_size as u64));
                            }

                            if self.system.display_all_images {
                                for image in images {
                                    image_stats(ui, image);
                                    ui.end_row();
                                }
                            } else {
                                for image in images.iter().take(MAX_ITEM_COUNT) {
                                    image_stats(ui, image);
                                    ui.end_row();
                                }
                            };
                        });
                    if !self.system.display_all_images && count > MAX_ITEM_COUNT {
                        ui.add(egui::Label::new("More to load...").weak());
                    }
                }
            });
    }

    fn build_cache_usage(&mut self, ui: &mut egui::Ui) {
        if let Some(usage) = self.system.data_usage.as_ref() {
            CollapsingHeader::new("build cache")
                .default_open(false)
                .show(ui, |ui| {
                    if let Some(cache) = &usage.build_cache {
                        let (count, total) = usage.images.iter().fold((0, 0), |mut acc, i| {
                            acc.0 += 1;
                            acc.1 += i.size;
                            acc
                        });
                        ui.checkbox(&mut self.system.display_all_cache, "Display all");
                        ui.add_space(10.);
                        key_val!(ui, "Total size:", crate::conv_b(total as u64));
                        Grid::new("cache_grid")
                            .spacing((20., 10.))
                            .min_col_width(50.)
                            .striped(true)
                            .show(ui, |ui| {
                                key!(ui, "ID");
                                key!(ui, "Type");
                                key!(ui, "Created");
                                key!(ui, "In use");
                                key!(ui, "Shared");
                                key!(ui, "Usage count");
                                key!(ui, "Size");
                                ui.end_row();
                                let mut images = cache.iter().collect::<Vec<_>>();
                                images.sort_by(|a, b| b.size.cmp(&a.size));

                                fn cache_stats(
                                    ui: &mut egui::Ui,
                                    cache: &docker_api::api::BuildCache,
                                ) {
                                    ui.scope(|ui| {
                                        ui.add(egui::Label::new(icon::DISK).heading().strong());
                                        val!(ui, images::trim_id(&cache.id));
                                    });
                                    val!(ui, &cache.type_);
                                    val!(ui, cache.created_at.to_rfc2822());
                                    val!(ui, cache.in_use);
                                    val!(ui, cache.shared);
                                    val!(ui, cache.usage_count);
                                    val!(ui, crate::conv_b(cache.size as u64));
                                }

                                if self.system.display_all_cache {
                                    for cache in images {
                                        cache_stats(ui, cache);
                                        ui.end_row();
                                    }
                                } else {
                                    for cache in images.iter().take(MAX_ITEM_COUNT) {
                                        cache_stats(ui, cache);
                                        ui.end_row();
                                    }
                                };
                            });
                        if !self.system.display_all_cache && count > MAX_ITEM_COUNT {
                            ui.add(egui::Label::new("More to load...").weak());
                        }
                    }
                });
        }
    }

    fn system_details(&mut self, ui: &mut egui::Ui) {
        if let Some(system) = &self.system.system_info {
            ui.allocate_space((f32::INFINITY, 0.).into());
            Grid::new("basic_info_grid").show(ui, |ui| {
                key_val!(ui, "Version:", &system.version.version);
                key_val!(ui, "API version:", &system.version.api_version);
                key_val!(ui, "OS type:", &system.info.os_type);
                if !system.info.operating_system.is_empty() {
                    key_val!(ui, "OS:", &system.info.operating_system);
                }
                if !system.info.os_version.is_empty() {
                    key_val!(ui, "OS version:", &system.info.os_version);
                }
                key_val!(ui, "Architecture:", &system.version.arch);
                key_val!(ui, "Kernel version:", &system.version.kernel_version);
                key_val!(ui, "Go version:", &system.version.go_version);
                key_val!(ui, "Git commit:", &system.version.git_commit);
                key_val!(ui, "Build time:", system.version.build_time.to_rfc2822());

                if let Some(labels) = &system.info.labels {
                    key!(ui, "Labels:");
                    ui.end_row();
                    ui.label("          ");
                    Grid::new("labels_grid").show(ui, |ui| {
                        let mut labels = labels.iter().collect::<Vec<_>>();
                        labels.sort();
                        for label in labels {
                            val!(ui, &label);
                            ui.end_row();
                        }
                    });
                    ui.end_row();
                }
            });

            CollapsingHeader::new("details")
                .default_open(false)
                .show(ui, |ui| {
                    Grid::new("details_grid").show(ui, |ui| {
                        key_val!(ui, "Experimental build:", system.info.experimental_build);
                        key_val!(ui, "Debug:", system.info.debug);
                        key_val!(ui, "Driver:", &system.info.driver);
                        key_val!(ui, "Logging driver:", &system.info.logging_driver);
                        key_val!(ui, "CGroup driver:", &system.info.cgroup_driver);
                        key_val!(ui, "CGroup version:", &system.info.cgroup_version);
                        key_val!(ui, "Init binary:", &system.info.init_binary);
                        key_val!(ui, "Root directory:", &system.info.docker_root_dir);
                        key_val!(ui, "Isolation:", system.info.isolation.as_ref());
                        key_val!(ui, "Pids limit:", system.info.pids_limit);
                        if let Some(license) = system.info.product_license.as_ref() {
                            key_val!(ui, "License:", &license);
                        }
                        if !system.info.security_options.is_empty() {
                            key!(ui, "Security options:");
                            ui.end_row();
                            ui.label("          ");
                            Grid::new("sec_opts_grid").show(ui, |ui| {
                                let mut opts =
                                    system.info.security_options.iter().collect::<Vec<_>>();
                                opts.sort();
                                for opt in opts {
                                    val!(ui, &opt);
                                    ui.end_row();
                                }
                            });
                            ui.end_row();
                        }
                    });
                    CollapsingHeader::new("CPU")
                        .default_open(false)
                        .show(ui, |ui| {
                            Grid::new("cpus_grid").show(ui, |ui| {
                                key_val!(ui, "CPUs:", system.info.n_cpu);
                                key_val!(ui, "CPU CFS period:", system.info.cpu_cfs_period);
                                key_val!(ui, "CPU CFS quota:", system.info.cpu_cfs_quota);
                                key_val!(ui, "CPU shares:", system.info.cpu_shares);
                                key_val!(ui, "CPU set:", system.info.cpu_set);
                            });
                        });
                    CollapsingHeader::new("memory")
                        .default_open(false)
                        .show(ui, |ui| {
                            Grid::new("mem_grid").show(ui, |ui| {
                                key_val!(ui, "Total memory:", crate::conv_b(system.info.mem_total));
                                key_val!(ui, "Memory limit:", system.info.memory_limit);
                                key_val!(ui, "Swap limit:", system.info.swap_limit);
                                key_val!(ui, "OOM kill disable:", system.info.oom_kill_disable);
                            });
                        });
                    CollapsingHeader::new("network")
                        .default_open(false)
                        .show(ui, |ui| {
                            Grid::new("net_grid").show(ui, |ui| {
                                key_val!(ui, "HTTP proxy:", &system.info.http_proxy);
                                key_val!(ui, "HTTPS proxy:", &system.info.https_proxy);
                                key_val!(ui, "No proxy:", &system.info.no_proxy);
                                key_val!(ui, "IPv4 forwarding:", system.info.ipv4_forwarding);
                                key_val!(ui, "Bridge nf iptables:", system.info.bridge_nf_iptables);
                                key_val!(
                                    ui,
                                    "Bridge nf ip6tables:",
                                    system.info.bridge_nf_ip6tables
                                );
                            });
                        });
                    CollapsingHeader::new("counters")
                        .default_open(false)
                        .show(ui, |ui| {
                            Grid::new("counters_grid").show(ui, |ui| {
                                key_val!(ui, "Total containers:", system.info.containers);
                                key_val!(ui, "Running containers:", system.info.containers_running);
                                key_val!(ui, "Paused containers:", system.info.containers_paused);
                                key_val!(ui, "Stopped containers:", system.info.containers_stopped);
                                key_val!(ui, "Images:", system.info.images);
                                key_val!(ui, "File descriptors:", system.info.n_fd);
                                key_val!(ui, "Go routines:", system.info.n_goroutines);
                                key_val!(ui, "Event listeners:", system.info.n_goroutines);
                            });
                        });
                });
        }
    }
}

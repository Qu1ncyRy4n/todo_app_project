//! A fixed, multi-generation fixture for deciding how family structure reads.
//!
//! The fixture never changes its cards or positions when a rendering choice
//! changes. Otherwise a prettier layout could be mistaken for a better visual
//! treatment of the very same parent/child relationships.

use std::collections::{BTreeSet, HashSet};

use eframe::egui::{self, Align2, Color32, FontId, Pos2, Rect, Sense, Stroke, Vec2};

#[derive(Clone, Copy, Default, Eq, PartialEq)]
enum LabPage {
    First,
    Dynamic,
    TagHierarchy,
    #[default]
    Tags,
}

#[derive(Clone, Copy, Default, Eq, PartialEq)]
enum TagStep {
    Scope,
    #[default]
    HierarchyColor,
    CompetingTags,
    Connections,
}

#[derive(Clone, Copy, Default, Eq, PartialEq)]
enum TagScope {
    DeepestOnly,
    #[default]
    ParentAndChild,
    ParentRail,
}

#[derive(Clone, Copy, Default, Eq, PartialEq)]
enum HierarchyColor {
    SameHue,
    #[default]
    DepthShade,
    BranchHue,
}

#[derive(Clone, Copy, Default, Eq, PartialEq)]
enum CompetingTagTreatment {
    Blend,
    #[default]
    Foreground,
    Cutout,
}

#[derive(Clone, Copy, Default, Eq, PartialEq)]
enum CloudConnections {
    #[default]
    MinimumTree,
    NearbyPairs,
    DenseNeighborhood,
}

#[derive(Clone, Copy, Default, Eq, PartialEq)]
enum NeighborLinks {
    #[default]
    Nearest,
    WithinRadius,
}

#[derive(Clone, Copy, Default, Eq, PartialEq)]
enum CompetingTagPosition {
    Blend,
    #[default]
    Foreground,
}

#[derive(Clone, Copy, Default, Eq, PartialEq)]
enum Fixture {
    #[default]
    ParentAbove,
    ParentBetween,
}

#[derive(Clone, Copy, Default, Eq, PartialEq)]
enum Structure {
    Tree,
    Box,
    #[default]
    Hybrid,
}

#[derive(Clone, Copy, Default, Eq, PartialEq)]
enum Attachment {
    #[default]
    Tab,
    Anchor,
    Integrated,
}

#[derive(Clone, Copy, Default, Eq, PartialEq)]
enum ChildSpace {
    Open,
    #[default]
    Recessed,
    Nested,
}

#[derive(Clone, Copy, Default, Eq, PartialEq)]
enum Remote {
    #[default]
    Outside,
    Span,
    Portal,
}

pub(super) struct LabState {
    page: LabPage,
    step: usize,
    fixture: Fixture,
    structure: Structure,
    attachment: Attachment,
    child_space: ChildSpace,
    remote: Remote,
    show_tags: bool,
    min_box_members: usize,
    cluster_range: f32,
    detached_distance: f32,
    attachment_width: f32,
    nested_families: bool,
    child_background: bool,
    tag_step: TagStep,
    tag_scope: TagScope,
    hierarchy_color: HierarchyColor,
    competing_tags: CompetingTagTreatment,
    cloud_connections: CloudConnections,
    complex_tag_step: TagStep,
    complex_tag_depth: usize,
    complex_neighbor_count: usize,
    complex_neighbor_links: NeighborLinks,
    complex_neighbor_radius: f32,
    complex_competing_position: CompetingTagPosition,
    complex_cutout: bool,
    complex_competing_outline: bool,
    complex_fill_local_gaps: bool,
}

impl Default for LabState {
    fn default() -> Self {
        Self {
            page: LabPage::Dynamic,
            step: 0,
            fixture: Fixture::ParentAbove,
            structure: Structure::Hybrid,
            attachment: Attachment::Tab,
            child_space: ChildSpace::Recessed,
            remote: Remote::Outside,
            show_tags: false,
            min_box_members: 2,
            cluster_range: 190.0,
            detached_distance: 300.0,
            attachment_width: 0.22,
            nested_families: true,
            child_background: true,
            tag_step: TagStep::HierarchyColor,
            tag_scope: TagScope::ParentAndChild,
            hierarchy_color: HierarchyColor::BranchHue,
            competing_tags: CompetingTagTreatment::Foreground,
            cloud_connections: CloudConnections::MinimumTree,
            complex_tag_step: TagStep::HierarchyColor,
            complex_tag_depth: 0,
            complex_neighbor_count: 5,
            complex_neighbor_links: NeighborLinks::Nearest,
            complex_neighbor_radius: 280.0,
            complex_competing_position: CompetingTagPosition::Foreground,
            complex_cutout: true,
            complex_competing_outline: false,
            complex_fill_local_gaps: true,
        }
    }
}

const STEPS: [&str; 4] = [
    "1 · Family structure",
    "2 · Parent attachment",
    "3 · Child space",
    "4 · Distant child",
];

pub(super) fn show(ctx: &egui::Context, open: &mut bool, state: &mut LabState) {
    if !*open {
        return;
    }
    egui::Window::new("Graph design tree")
        .open(open)
        .default_size(Vec2::new(1190.0, 790.0))
        .min_width(760.0)
        .resizable(true)
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.selectable_value(&mut state.page, LabPage::First, "First set · structure");
                ui.selectable_value(&mut state.page, LabPage::Dynamic, "Next set · dynamic families");
                ui.selectable_value(&mut state.page, LabPage::Tags, "Tag clouds · decision tree");
                ui.selectable_value(
                    &mut state.page,
                    LabPage::TagHierarchy,
                    "Next set · tag hierarchy",
                );
            });
            ui.separator();
            if state.page == LabPage::TagHierarchy {
                show_complex_tag_controls(ui, state);
                return;
            }
            if state.page == LabPage::Tags {
                show_tag_controls(ui, state);
                return;
            }
            if state.page == LabPage::Dynamic {
                show_dynamic_controls(ui, state);
                return;
            }
            ui.label("One fixed graph; each choice changes one visual rule, never task positions or parenthood.");
            ui.horizontal(|ui| {
                ui.label("Fixture:");
                ui.selectable_value(&mut state.fixture, Fixture::ParentAbove, "Parent above children");
                ui.selectable_value(&mut state.fixture, Fixture::ParentBetween, "Parent between children");
                ui.separator();
                ui.checkbox(&mut state.show_tags, "Tag context");
            });
            ui.separator();
            ui.horizontal_top(|ui| {
                ui.vertical(|ui| {
                    ui.set_width(252.0);
                    for (index, title) in STEPS.iter().enumerate() {
                        ui.selectable_value(&mut state.step, index, *title);
                    }
                    ui.separator();
                    match state.step {
                        0 => {
                            ui.label("Does a family need a containing outline?");
                            ui.radio_value(&mut state.structure, Structure::Tree, "Tree only");
                            ui.radio_value(&mut state.structure, Structure::Box, "Box + tree");
                            ui.radio_value(&mut state.structure, Structure::Hybrid, "Local box + remote tree");
                            ui.small("Tree only reprises the earlier independent family tree. Box + tree reprises contained children. Hybrid keeps nearby children inside and connects remote work outside.");
                        }
                        1 => {
                            ui.label("How does the parent card own its outline?");
                            ui.add_enabled_ui(state.structure != Structure::Tree, |ui| {
                                ui.radio_value(&mut state.attachment, Attachment::Tab, "Attached tab");
                                ui.radio_value(&mut state.attachment, Attachment::Anchor, "Explicit outline anchor");
                                ui.radio_value(&mut state.attachment, Attachment::Integrated, "Continuous card-to-rim");
                            });
                            ui.small("The anchor is the thick bridge to the frame. It is not another parent/child edge.");
                        }
                        2 => {
                            ui.label("What makes the child subgraph feel inside?");
                            ui.add_enabled_ui(state.structure != Structure::Tree, |ui| {
                                ui.radio_value(&mut state.child_space, ChildSpace::Open, "Open well");
                                ui.radio_value(&mut state.child_space, ChildSpace::Recessed, "Recessed well");
                                ui.radio_value(&mut state.child_space, ChildSpace::Nested, "Nested family");
                            });
                            ui.small("The nested option gives the child with a child its own smaller compartment.");
                        }
                        _ => {
                            ui.label("What happens when one child is far away?");
                            ui.add_enabled_ui(state.structure == Structure::Hybrid, |ui| {
                                ui.radio_value(&mut state.remote, Remote::Outside, "Outside + true branch");
                                ui.radio_value(&mut state.remote, Remote::Span, "Expand frame to child");
                                ui.radio_value(&mut state.remote, Remote::Portal, "Named portal in frame");
                            });
                            ui.small("Outside leaves the remote task where it is. Span deliberately shows the costly large box. Portal is a preview, not the actual card.");
                            if state.structure != Structure::Hybrid { ui.weak("This decision applies to Hybrid. Box always spans; Tree always leaves the child outside."); }
                        }
                    }
                    ui.separator();
                    ui.horizontal(|ui| {
                        if ui.add_enabled(state.step > 0, egui::Button::new("← Back")).clicked() {
                            state.step -= 1;
                        }
                        if ui.add_enabled(state.step + 1 < STEPS.len(), egui::Button::new("Next →")).clicked() {
                            state.step += 1;
                        }
                    });
                    ui.separator();
                    ui.label("Current path");
                    ui.small(format!("{} → {} → {} → {}", structure_name(state.structure), attachment_name(state.attachment), space_name(state.child_space), remote_name(state.remote)));
                });
                ui.separator();
                ui.vertical(|ui| {
                    let width = ui.available_width().max(470.0);
                    ui.heading("Same cards · same positions · same relationships");
                    let size = Vec2::new(width, (width * 0.68).clamp(430.0, 650.0));
                    let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
                    draw_scene(&ui.painter_at(rect), rect, state);
                    ui.small("Thick dark rim = parent outline · angular dark line = family tree · blue arrow = dependency · pale circles = tags. Remote task and nested grandchild remain visible in every choice.");
                });
            });
        });
}

fn structure_name(value: Structure) -> &'static str {
    match value {
        Structure::Tree => "Tree",
        Structure::Box => "Box",
        Structure::Hybrid => "Hybrid",
    }
}
fn attachment_name(value: Attachment) -> &'static str {
    match value {
        Attachment::Tab => "Tab",
        Attachment::Anchor => "Anchor",
        Attachment::Integrated => "Continuous",
    }
}
fn space_name(value: ChildSpace) -> &'static str {
    match value {
        ChildSpace::Open => "Open",
        ChildSpace::Recessed => "Recessed",
        ChildSpace::Nested => "Nested",
    }
}
fn remote_name(value: Remote) -> &'static str {
    match value {
        Remote::Outside => "Outside",
        Remote::Span => "Span",
        Remote::Portal => "Portal",
    }
}

const CANVAS: Color32 = Color32::from_rgb(248, 249, 251);
const FRAME: Color32 = Color32::from_rgb(57, 83, 103);
const FRAME_EDGE: Color32 = Color32::from_rgb(125, 172, 190);
const WELL: Color32 = Color32::from_rgb(226, 239, 243);
const CARD: Color32 = Color32::from_rgb(54, 65, 99);
const CARD_EDGE: Color32 = Color32::from_rgb(85, 199, 214);
const TAG_CYAN: Color32 = Color32::from_rgb(220, 239, 241);
const TAG_CORAL: Color32 = Color32::from_rgb(245, 229, 223);
const DEPENDENCY: Color32 = Color32::from_rgb(66, 143, 219);

struct Scene<'a> {
    painter: &'a egui::Painter,
    origin: Pos2,
    scale: f32,
}

impl Scene<'_> {
    fn at(&self, x: f32, y: f32) -> Pos2 {
        self.origin + Vec2::new(x, y) * self.scale
    }
    fn rect(&self, x: f32, y: f32, w: f32, h: f32) -> Rect {
        Rect::from_min_size(self.at(x, y), Vec2::new(w, h) * self.scale)
    }
    fn stroke(&self, width: f32, color: Color32) -> Stroke {
        Stroke::new(width * self.scale, color)
    }
    fn line(&self, points: &[(f32, f32)], width: f32, color: Color32) {
        self.painter.add(egui::Shape::line(
            points.iter().map(|(x, y)| self.at(*x, *y)).collect(),
            self.stroke(width, color),
        ));
    }
    fn circle(&self, x: f32, y: f32) {
        self.painter
            .circle_filled(self.at(x, y), 4.0 * self.scale, CANVAS);
        self.painter.circle_stroke(
            self.at(x, y),
            4.0 * self.scale,
            self.stroke(1.7, FRAME_EDGE),
        );
    }
    fn label(&self, x: f32, y: f32, label: &str, color: Color32) {
        self.painter.text(
            self.at(x, y),
            Align2::LEFT_CENTER,
            label,
            FontId::proportional(12.0 * self.scale),
            color,
        );
    }
    fn card(&self, rect: Rect, title: &str, parent: bool) {
        self.painter.rect_filled(rect, 9.0 * self.scale, CARD);
        self.painter.rect_stroke(
            rect,
            9.0 * self.scale,
            self.stroke(if parent { 3.0 } else { 2.0 }, CARD_EDGE),
            egui::StrokeKind::Outside,
        );
        self.painter.text(
            rect.center(),
            Align2::CENTER_CENTER,
            title,
            FontId::proportional(15.0 * self.scale),
            Color32::WHITE,
        );
    }
}

fn draw_scene(painter: &egui::Painter, canvas: Rect, state: &LabState) {
    let scale = (canvas.width() / 900.0).min(canvas.height() / 610.0);
    let scene = Scene {
        painter,
        origin: canvas.min,
        scale,
    };
    painter.rect_filled(canvas, 8.0, CANVAS);
    let parent = match state.fixture {
        Fixture::ParentAbove => scene.rect(375.0, 35.0, 170.0, 63.0),
        Fixture::ParentBetween => scene.rect(375.0, 207.0, 170.0, 63.0),
    };
    let child_a = scene.rect(157.0, 250.0, 165.0, 65.0);
    let child_b = scene.rect(426.0, 250.0, 165.0, 65.0);
    let remote = scene.rect(710.0, 305.0, 165.0, 65.0);
    let grandchild = scene.rect(426.0, 455.0, 165.0, 65.0);
    let neighbor = scene.rect(35.0, 468.0, 160.0, 65.0);
    let framed = state.structure != Structure::Tree;
    let span = state.structure == Structure::Box
        || (state.structure == Structure::Hybrid && state.remote == Remote::Span);
    let frame = scene.rect(112.0, 169.0, if span { 776.0 } else { 530.0 }, 390.0);
    let nested = scene.rect(389.0, 352.0, 239.0, 190.0);

    if state.show_tags {
        for (rect, color) in [
            (parent, TAG_CYAN),
            (child_a, TAG_CYAN),
            (child_b, TAG_CYAN),
            (grandchild, TAG_CYAN),
            (remote, TAG_CYAN),
            (neighbor, TAG_CORAL),
        ] {
            painter.circle_filled(rect.center(), 76.0 * scale, color);
        }
        scene.label(22.0, 28.0, "work / graph", FRAME_EDGE);
        scene.label(34.0, 555.0, "planning", Color32::from_rgb(182, 116, 95));
    }

    if framed {
        // A frame is allowed to claim only the cards actually inside it. In
        // the remote case the far card stays outside unless Span is selected.
        painter.rect_filled(frame, 19.0 * scale, FRAME);
        let well = frame.shrink(17.0 * scale);
        painter.rect_filled(
            well,
            12.0 * scale,
            match state.child_space {
                ChildSpace::Open => CANVAS,
                ChildSpace::Recessed | ChildSpace::Nested => WELL,
            },
        );
        painter.rect_stroke(
            frame,
            19.0 * scale,
            scene.stroke(4.0, FRAME_EDGE),
            egui::StrokeKind::Outside,
        );
        if state.child_space == ChildSpace::Nested {
            painter.rect_filled(nested, 12.0 * scale, FRAME);
            painter.rect_filled(nested.shrink(12.0 * scale), 8.0 * scale, WELL);
            painter.rect_stroke(
                nested,
                12.0 * scale,
                scene.stroke(2.0, FRAME_EDGE),
                egui::StrokeKind::Outside,
            );
        }
        draw_attachment(&scene, parent, frame, state.attachment, state.fixture);
    }

    // Shared angular trunk and junction: no second diagonal parent edge.
    let (start_x, start_y) = match state.fixture {
        Fixture::ParentAbove => (460.0, if framed { 169.0 } else { 98.0 }),
        Fixture::ParentBetween => (460.0, 270.0),
    };
    let branch_y = if state.fixture == Fixture::ParentAbove {
        216.0
    } else {
        286.0
    };
    scene.line(
        &[
            (start_x, start_y),
            (start_x, branch_y),
            (239.0, branch_y),
            (239.0, 250.0),
        ],
        4.0,
        FRAME,
    );
    scene.line(
        &[(239.0, branch_y), (508.0, branch_y), (508.0, 250.0)],
        4.0,
        FRAME,
    );
    scene.circle(239.0, 250.0);
    scene.circle(508.0, 250.0);
    scene.line(&[(508.0, 315.0), (508.0, 455.0)], 3.3, FRAME);
    scene.circle(508.0, 455.0);

    match (state.structure, state.remote) {
        (Structure::Hybrid, Remote::Portal) => {
            let portal = scene.rect(143.0, 385.0, 175.0, 43.0);
            painter.rect_filled(portal, 8.0 * scale, WELL);
            painter.rect_stroke(
                portal,
                8.0 * scale,
                scene.stroke(2.0, FRAME_EDGE),
                egui::StrokeKind::Outside,
            );
            scene.label(151.0, 406.0, "↗ distant child preview", FRAME);
            scene.label(680.0, 390.0, "actual card ↗", FRAME);
        }
        _ => {
            // With a bounded frame, the line crosses the rim rather than
            // implying that the remote card is inside a compact child well.
            scene.line(
                &[(508.0, branch_y), (792.0, branch_y), (792.0, 305.0)],
                3.3,
                FRAME,
            );
            scene.circle(792.0, 305.0);
            if framed && !span {
                scene.label(656.0, 387.0, "outside this family outline", FRAME);
            }
        }
    }
    if state.fixture == Fixture::ParentBetween {
        scene.label(366.0, 291.0, "parent among children", FRAME);
    }

    painter.arrow(
        child_a.right_center(),
        child_b.left_center() - child_a.right_center() - Vec2::new(8.0 * scale, 0.0),
        scene.stroke(2.2, DEPENDENCY),
    );
    painter.arrow(
        neighbor.right_center(),
        grandchild.left_center() - neighbor.right_center() - Vec2::new(8.0 * scale, 0.0),
        scene.stroke(2.2, DEPENDENCY),
    );
    scene.card(child_a, "near child A", false);
    scene.card(child_b, "child B / parent", true);
    scene.card(remote, "distant child", false);
    scene.card(grandchild, "grandchild", false);
    scene.card(neighbor, "other family", false);
    scene.card(parent, "parent task", true);
}

fn draw_attachment(
    scene: &Scene<'_>,
    parent: Rect,
    frame: Rect,
    attachment: Attachment,
    fixture: Fixture,
) {
    let x = parent.center().x;
    let top = frame.top();
    let (from, to) = if fixture == Fixture::ParentBetween {
        (top, parent.top())
    } else {
        (parent.bottom(), top)
    };
    match attachment {
        Attachment::Tab => {
            let bridge = Rect::from_min_max(
                Pos2::new(x - 30.0 * scene.scale, from),
                Pos2::new(x + 30.0 * scene.scale, to),
            );
            scene.painter.rect_filled(bridge, 0.0, FRAME);
        }
        Attachment::Anchor => {
            let target = Pos2::new(x, top);
            let source = if fixture == Fixture::ParentBetween {
                parent.center_top()
            } else {
                parent.center_bottom()
            };
            scene
                .painter
                .line_segment([source, target], scene.stroke(13.0, FRAME));
            scene
                .painter
                .line_segment([source, target], scene.stroke(2.0, FRAME_EDGE));
        }
        Attachment::Integrated => {
            let bridge = Rect::from_min_max(
                Pos2::new(parent.left() + 5.0 * scene.scale, from),
                Pos2::new(parent.right() - 5.0 * scene.scale, to),
            );
            scene.painter.rect_filled(bridge, 0.0, FRAME);
            scene.painter.line_segment(
                [bridge.left_top(), bridge.left_bottom()],
                scene.stroke(3.0, FRAME_EDGE),
            );
            scene.painter.line_segment(
                [bridge.right_top(), bridge.right_bottom()],
                scene.stroke(3.0, FRAME_EDGE),
            );
        }
    }
}

fn show_dynamic_controls(ui: &mut egui::Ui, state: &mut LabState) {
    ui.label("Round 2: fixed tasks and positions. These controls are a visual experiment, not saved theme settings yet.");
    ui.horizontal_top(|ui| {
        ui.vertical(|ui| {
            ui.set_width(260.0);
            ui.heading("Dynamic family rule");
            ui.add(egui::Slider::new(&mut state.min_box_members, 2..=4).text("n · children per box"));
            ui.add(egui::Slider::new(&mut state.cluster_range, 110.0..=300.0).text("m · join distance"));
            ui.small("Direct children join a cluster when their card centers are within m. A cluster gets a box only when it contains at least n direct children.");
            ui.separator();
            ui.heading("How a box joins its parent");
            ui.add(egui::Slider::new(&mut state.detached_distance, 180.0..=520.0).text("near / far threshold"));
            ui.add(egui::Slider::new(&mut state.attachment_width, 0.0..=1.5).text("rim join / card width"));
            ui.small("Near boxes join the parent card to the rim. Far boxes use an ordinary angular family-tree branch to the rim. A card inside its own box needs no extra join.");
            ui.separator();
            ui.heading("Inside the box");
            ui.checkbox(&mut state.nested_families, "Nested family frames");
            ui.checkbox(&mut state.child_background, "Child-space background");
            ui.checkbox(&mut state.show_tags, "Show tag context");
            ui.small("Nested families and background are independent. Both are on by default here.");
        });
        ui.separator();
        ui.vertical(|ui| {
            let width = ui.available_width().max(470.0);
            ui.heading("Connected box · detached box · tree-only children");
            let size = Vec2::new(width, (width * 0.68).clamp(430.0, 650.0));
            let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
            draw_dynamic_scene(&ui.painter_at(rect), rect, state);
            ui.small("The same five direct children and one grandchild stay fixed. Blue arrow = dependency; dark angular branches and open terminals = parenthood.");
        });
    });
}

/// Transitive proximity is intentional: A near B and B near C form one child
/// subgraph even when A and C are farther apart than the slider value.
fn proximity_clusters(cards: &[Rect], range: f32) -> Vec<Vec<usize>> {
    let mut seen = HashSet::new();
    let mut groups = Vec::new();
    for start in 0..cards.len() {
        if !seen.insert(start) {
            continue;
        }
        let mut group = vec![start];
        let mut cursor = 0;
        while let Some(index) = group.get(cursor).copied() {
            let Some(card) = cards.get(index) else { break };
            for (candidate, other) in cards.iter().enumerate() {
                if card.center().distance(other.center()) <= range && seen.insert(candidate) {
                    group.push(candidate);
                }
            }
            cursor += 1;
        }
        groups.push(group);
    }
    groups
}

fn draw_dynamic_scene(painter: &egui::Painter, canvas: Rect, state: &LabState) {
    let scale = (canvas.width() / 900.0).min(canvas.height() / 610.0);
    let scene = Scene {
        painter,
        origin: canvas.min,
        scale,
    };
    painter.rect_filled(canvas, 8.0, CANVAS);
    let parent = scene.rect(405.0, 30.0, 170.0, 70.0);
    let cards = [
        scene.rect(125.0, 245.0, 120.0, 60.0),
        scene.rect(285.0, 245.0, 120.0, 60.0),
        scene.rect(445.0, 245.0, 120.0, 60.0),
        scene.rect(690.0, 245.0, 120.0, 60.0),
        scene.rect(745.0, 355.0, 120.0, 60.0),
    ];
    let grandchild = scene.rect(285.0, 445.0, 120.0, 60.0);
    let neighbor = scene.rect(45.0, 480.0, 120.0, 60.0);
    if state.show_tags {
        for card in [
            parent, cards[0], cards[1], cards[2], cards[3], cards[4], grandchild,
        ] {
            painter.circle_filled(card.center(), 68.0 * scale, TAG_CYAN);
        }
        painter.circle_filled(neighbor.center(), 68.0 * scale, TAG_CORAL);
    }

    let mut tree_targets = Vec::new();
    let mut box_count = 0;
    let mut tree_count = 0;
    for group in proximity_clusters(&cards, state.cluster_range * scale) {
        if group.len() < state.min_box_members {
            for index in group {
                if let Some(card) = cards.get(index) {
                    tree_targets.push(card.center_top());
                    tree_count += 1;
                }
            }
            continue;
        }
        let direct = group
            .iter()
            .filter_map(|index| cards.get(*index))
            .copied()
            .collect::<Vec<_>>();
        let count = f32::from(u8::try_from(direct.len()).unwrap_or(u8::MAX));
        let centroid = direct
            .iter()
            .fold(Vec2::ZERO, |sum, card| sum + card.center().to_vec2())
            / count;
        let mut bounds = direct
            .iter()
            .fold(Rect::NOTHING, |rect, card| rect.union(*card));
        // Turning off the nested border must not eject the grandchild from
        // its outer family: containment and decoration are separate choices.
        if group.contains(&1) {
            bounds = bounds.union(grandchild);
        }
        let frame = Rect::from_min_max(
            bounds.min - Vec2::new(20.0, 60.0) * scale,
            bounds.max + Vec2::splat(20.0 * scale),
        );
        let center = Pos2::new(centroid.x, centroid.y);
        // A continuous rim join is only honest when the card projects onto
        // the frame. A remote box off to one side keeps the tree vocabulary.
        let parent_inside = frame.contains(parent.min) && frame.contains(parent.max);
        let connected = parent_inside
            || (parent.center().distance(center) <= state.detached_distance * scale
                && parent.left() <= frame.right()
                && parent.right() >= frame.left());
        draw_dynamic_frame(&scene, frame, state.child_background);
        let label = if parent_inside {
            "PARENT INSIDE BOX"
        } else if connected {
            "CONNECTED BOX"
        } else {
            "DETACHED BOX"
        };
        scene.painter.text(
            frame.left_top() + Vec2::new(12.0, 11.0) * scale,
            Align2::LEFT_CENTER,
            format!("{label} · {} children", group.len()),
            FontId::proportional(12.0 * scale),
            Color32::WHITE,
        );
        if connected && !parent_inside {
            let half = (parent.width() * state.attachment_width * 0.5).max(1.5 * scale);
            let anchor_x = parent.center().x.clamp(frame.left(), frame.right());
            let bridge = Rect::from_min_max(
                Pos2::new((anchor_x - half).max(frame.left()), parent.bottom()),
                Pos2::new((anchor_x + half).min(frame.right()), frame.top()),
            );
            painter.rect_filled(bridge, 0.0, FRAME);
            painter.line_segment(
                [bridge.left_top(), bridge.left_bottom()],
                scene.stroke(1.5, FRAME_EDGE),
            );
            painter.line_segment(
                [bridge.right_top(), bridge.right_bottom()],
                scene.stroke(1.5, FRAME_EDGE),
            );
        } else {
            tree_targets.push(frame.center_top());
        }
        let junction = Pos2::new(frame.center().x, frame.top() + 39.0 * scale);
        painter.line_segment([frame.center_top(), junction], scene.stroke(3.5, FRAME));
        for card in direct {
            let corner = Pos2::new(card.center().x, junction.y);
            scene.painter.add(egui::Shape::line(
                vec![junction, corner, card.center_top()],
                scene.stroke(3.5, FRAME),
            ));
            painter.circle_stroke(
                card.center_top(),
                4.0 * scale,
                scene.stroke(1.7, FRAME_EDGE),
            );
        }
        box_count += 1;
    }

    if !tree_targets.is_empty() {
        let junction = scene.at(490.0, 157.0);
        painter.line_segment([parent.center_bottom(), junction], scene.stroke(3.5, FRAME));
        painter.circle_stroke(junction, 4.0 * scale, scene.stroke(1.7, FRAME_EDGE));
        for target in tree_targets {
            let corner = Pos2::new(target.x, junction.y);
            painter.add(egui::Shape::line(
                vec![junction, corner, target],
                scene.stroke(3.5, FRAME),
            ));
            painter.circle_stroke(target, 4.0 * scale, scene.stroke(1.7, FRAME_EDGE));
        }
    }

    if state.nested_families {
        let nested = scene.rect(265.0, 370.0, 160.0, 155.0);
        draw_dynamic_frame(&scene, nested, state.child_background);
        let half = (cards[1].width() * state.attachment_width * 0.5).max(1.5 * scale);
        painter.rect_filled(
            Rect::from_min_max(
                Pos2::new(cards[1].center().x - half, cards[1].bottom()),
                Pos2::new(cards[1].center().x + half, nested.top()),
            ),
            0.0,
            FRAME,
        );
        painter.line_segment(
            [nested.center_top(), grandchild.center_top()],
            scene.stroke(3.0, FRAME),
        );
    } else {
        painter.line_segment(
            [cards[1].center_bottom(), grandchild.center_top()],
            scene.stroke(3.0, FRAME),
        );
    }
    painter.circle_stroke(
        grandchild.center_top(),
        4.0 * scale,
        scene.stroke(1.7, FRAME_EDGE),
    );
    painter.arrow(
        neighbor.right_center(),
        cards[2].left_center() - neighbor.right_center() - Vec2::new(7.0 * scale, 0.0),
        scene.stroke(2.0, DEPENDENCY),
    );

    scene.card(parent, "parent task", true);
    for (card, label) in cards.into_iter().zip([
        "child A",
        "child B / parent",
        "child C",
        "remote D",
        "remote E",
    ]) {
        scene.card(card, label, false);
    }
    scene.card(grandchild, "grandchild", false);
    scene.card(neighbor, "other family", false);
    scene.label(
        15.0,
        575.0,
        &format!("{box_count} boxes · {tree_count} direct tree-only children"),
        FRAME,
    );
}

fn draw_dynamic_frame(scene: &Scene<'_>, frame: Rect, background: bool) {
    let well = frame.shrink(13.0 * scene.scale);
    if background {
        scene.painter.rect_filled(frame, 15.0 * scene.scale, FRAME);
        scene.painter.rect_filled(well, 9.0 * scene.scale, WELL);
    } else {
        // An open well is a true hole. Filling it with the canvas color would
        // still erase the tag context that the toggle is meant to reveal.
        for band in [
            Rect::from_min_max(frame.min, Pos2::new(frame.right(), well.top())),
            Rect::from_min_max(Pos2::new(frame.left(), well.bottom()), frame.max),
            Rect::from_min_max(
                Pos2::new(frame.left(), well.top()),
                Pos2::new(well.left(), well.bottom()),
            ),
            Rect::from_min_max(
                Pos2::new(well.right(), well.top()),
                Pos2::new(frame.right(), well.bottom()),
            ),
        ] {
            scene.painter.rect_filled(band, 0.0, FRAME);
        }
    }
    scene.painter.rect_stroke(
        frame,
        15.0 * scene.scale,
        scene.stroke(3.0, FRAME_EDGE),
        egui::StrokeKind::Outside,
    );
}

const TAG_WORK_FIELD: Color32 = Color32::from_rgb(208, 235, 239);
const TAG_CDINT_SAME: Color32 = Color32::from_rgb(187, 222, 226);
const TAG_CDINT_DEPTH: Color32 = Color32::from_rgb(159, 205, 185);
const TAG_CDINT_BRANCH: Color32 = Color32::from_rgb(204, 187, 231);
const TAG_PERSONAL: Color32 = Color32::from_rgb(244, 213, 177);
const TAG_WORK_TEXT: Color32 = Color32::from_rgb(42, 119, 129);
const TAG_PERSONAL_TEXT: Color32 = Color32::from_rgb(166, 91, 53);

const TAG_STEPS: [&str; 4] = [
    "1 · Cloud scope",
    "2 · Hierarchy color",
    "3 · Competing tags",
    "4 · Neighbor links",
];

/// A second fixed decision tree for the tag system. It deliberately places a
/// personal/fun task inside the broad work field: every alternative therefore
/// has to make competing hierarchy visible without silently moving the task.
fn show_tag_controls(ui: &mut egui::Ui, state: &mut LabState) {
    ui.label("Fixed tag fixture: paths, positions, and card titles never change. These choices are a design experiment, not saved appearance settings.");
    ui.horizontal_top(|ui| {
        ui.vertical(|ui| {
            ui.set_width(272.0);
            for (index, title) in TAG_STEPS.iter().enumerate() {
                let step = match index {
                    0 => TagStep::Scope,
                    1 => TagStep::HierarchyColor,
                    2 => TagStep::CompetingTags,
                    _ => TagStep::Connections,
                };
                ui.selectable_value(&mut state.tag_step, step, *title);
            }
            ui.separator();
            match state.tag_step {
                TagStep::Scope => {
                    ui.label("Which hierarchy layers make cloud bodies?");
                    ui.radio_value(&mut state.tag_scope, TagScope::DeepestOnly, "Deepest path only");
                    ui.radio_value(&mut state.tag_scope, TagScope::ParentAndChild, "Broad parent + child cloud");
                    ui.radio_value(&mut state.tag_scope, TagScope::ParentRail, "Parent rail + child cloud");
                    ui.small("The broad choices let work remain a readable surrounding context while work/cdint remains its own local body.");
                }
                TagStep::HierarchyColor => {
                    ui.label("How should a descendant differ from its parent?");
                    ui.radio_value(&mut state.hierarchy_color, HierarchyColor::SameHue, "Same hue");
                    ui.radio_value(&mut state.hierarchy_color, HierarchyColor::DepthShade, "Depth shade");
                    ui.radio_value(&mut state.hierarchy_color, HierarchyColor::BranchHue, "Branch hue");
                    ui.small("Compare whether hierarchy reads through one family of color, depth, or a more distinct child identity.");
                }
                TagStep::CompetingTags => {
                    ui.label("What if another hierarchy occupies the broad field?");
                    ui.radio_value(&mut state.competing_tags, CompetingTagTreatment::Blend, "Blend with work");
                    ui.radio_value(&mut state.competing_tags, CompetingTagTreatment::Foreground, "Personal island in foreground");
                    ui.radio_value(&mut state.competing_tags, CompetingTagTreatment::Cutout, "Cut out the work field");
                    ui.small("The peach card is tag/personal/fun, not a work task. Foreground and cutout are candidates for the desired color push-away behavior.");
                }
                TagStep::Connections => {
                    ui.label("How densely should matching clouds join?");
                    ui.radio_value(&mut state.cloud_connections, CloudConnections::MinimumTree, "Minimum tree · current-like");
                    ui.radio_value(&mut state.cloud_connections, CloudConnections::NearbyPairs, "All nearby pairs");
                    ui.radio_value(&mut state.cloud_connections, CloudConnections::DenseNeighborhood, "Dense neighborhood");
                    ui.small("This affects only same-tag cloud rails. Structural family lines and dependency arrows stay separate.");
                }
            }
            ui.separator();
            ui.label("Current path");
            ui.small(format!(
                "{} → {} → {} → {}",
                tag_scope_name(state.tag_scope),
                hierarchy_color_name(state.hierarchy_color),
                competing_name(state.competing_tags),
                connection_name(state.cloud_connections),
            ));
        });
        ui.separator();
        ui.vertical(|ui| {
            let width = ui.available_width().max(470.0);
            ui.heading("Hierarchy cloud · distinct competing tag · neighbor density");
            let size = Vec2::new(width, (width * 0.67).clamp(430.0, 650.0));
            let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
            draw_tag_scene(&ui.painter_at(rect), rect, state);
            ui.small("Teal = tag/work · green or violet = tag/work/cdint variant · peach = tag/personal/fun. Cards stay above every cloud.");
        });
    });
}

fn tag_scope_name(value: TagScope) -> &'static str {
    match value {
        TagScope::DeepestOnly => "deepest only",
        TagScope::ParentAndChild => "parent + child",
        TagScope::ParentRail => "parent rail + child",
    }
}

fn hierarchy_color_name(value: HierarchyColor) -> &'static str {
    match value {
        HierarchyColor::SameHue => "same hue",
        HierarchyColor::DepthShade => "depth shade",
        HierarchyColor::BranchHue => "branch hue",
    }
}

fn competing_name(value: CompetingTagTreatment) -> &'static str {
    match value {
        CompetingTagTreatment::Blend => "blend",
        CompetingTagTreatment::Foreground => "foreground island",
        CompetingTagTreatment::Cutout => "cutout",
    }
}

fn connection_name(value: CloudConnections) -> &'static str {
    match value {
        CloudConnections::MinimumTree => "minimum tree",
        CloudConnections::NearbyPairs => "nearby pairs",
        CloudConnections::DenseNeighborhood => "dense neighborhood",
    }
}

fn child_cloud_color(mode: HierarchyColor) -> Color32 {
    match mode {
        HierarchyColor::SameHue => TAG_CDINT_SAME,
        HierarchyColor::DepthShade => TAG_CDINT_DEPTH,
        HierarchyColor::BranchHue => TAG_CDINT_BRANCH,
    }
}

fn cloud_link_pairs(centers: &[Pos2], mode: CloudConnections) -> Vec<(usize, usize)> {
    match mode {
        // This mirrors the current sparse one-body reading: enough links to
        // connect all members, but no redundant loops.
        CloudConnections::MinimumTree => vec![(0, 1), (1, 2), (2, 3), (3, 4)],
        CloudConnections::NearbyPairs => vec![(0, 1), (1, 2), (2, 3), (2, 4), (3, 4)],
        CloudConnections::DenseNeighborhood => {
            let mut links = Vec::new();
            for (left, from) in centers.iter().enumerate() {
                for (right, to) in centers.iter().enumerate().skip(left + 1) {
                    if from.distance(*to) <= 410.0 {
                        links.push((left, right));
                    }
                }
            }
            links
        }
    }
}

fn draw_cloud_links(
    painter: &egui::Painter,
    centers: &[Pos2],
    links: &[(usize, usize)],
    width: f32,
    color: Color32,
    scale: f32,
) {
    for (from, to) in links {
        let (Some(from), Some(to)) = (centers.get(*from), centers.get(*to)) else {
            continue;
        };
        painter.line_segment([*from, *to], Stroke::new(width * scale, color));
    }
}

fn draw_tag_scene(painter: &egui::Painter, canvas: Rect, state: &LabState) {
    let scale = (canvas.width() / 900.0).min(canvas.height() / 610.0);
    let scene = Scene {
        painter,
        origin: canvas.min,
        scale,
    };
    painter.rect_filled(canvas, 8.0, CANVAS);

    let cdint = [
        scene.rect(105.0, 145.0, 145.0, 62.0),
        scene.rect(290.0, 105.0, 145.0, 62.0),
        scene.rect(280.0, 315.0, 145.0, 62.0),
    ];
    let work = [
        cdint[0],
        cdint[1],
        cdint[2],
        scene.rect(520.0, 136.0, 145.0, 62.0),
        scene.rect(605.0, 330.0, 145.0, 62.0),
    ];
    // This card intentionally occupies the broad work field but belongs to a
    // competing tag hierarchy. It is the fixed negative/repulsion case.
    let personal = scene.rect(435.0, 225.0, 155.0, 62.0);
    let centers = work.iter().map(Rect::center).collect::<Vec<_>>();
    let child_centers = cdint.iter().map(Rect::center).collect::<Vec<_>>();
    let work_links = cloud_link_pairs(&centers, state.cloud_connections);
    let child_links = match state.cloud_connections {
        CloudConnections::MinimumTree => vec![(0, 1), (1, 2)],
        CloudConnections::NearbyPairs | CloudConnections::DenseNeighborhood => {
            vec![(0, 1), (1, 2), (0, 2)]
        }
    };

    if state.tag_scope != TagScope::DeepestOnly {
        draw_cloud_links(
            painter,
            &centers,
            &work_links,
            if state.tag_scope == TagScope::ParentRail {
                72.0
            } else {
                105.0
            },
            TAG_WORK_FIELD,
            scale,
        );
        for rect in &work {
            if state.tag_scope == TagScope::ParentRail {
                painter.rect_filled(rect.expand(20.0 * scale), 26.0 * scale, TAG_WORK_FIELD);
            } else {
                painter.circle_filled(rect.center(), 80.0 * scale, TAG_WORK_FIELD);
            }
        }
        scene.label(60.0, 61.0, "tag/work · broad context", TAG_WORK_TEXT);
    }

    // A foreground island is painted after the parent field; a cutout first
    // restores the canvas so the competing color never optically compounds.
    if state.competing_tags == CompetingTagTreatment::Cutout {
        painter.circle_filled(personal.center(), 92.0 * scale, CANVAS);
    }
    if state.competing_tags == CompetingTagTreatment::Blend {
        painter.circle_filled(personal.center(), 72.0 * scale, TAG_PERSONAL);
    }

    let child_color = child_cloud_color(state.hierarchy_color);
    draw_cloud_links(
        painter,
        &child_centers,
        &child_links,
        71.0,
        child_color,
        scale,
    );
    for rect in &cdint {
        painter.circle_filled(rect.center(), 57.0 * scale, child_color);
    }
    scene.label(
        94.0,
        410.0,
        "tag/work/cdint · local child cloud",
        TAG_WORK_TEXT,
    );

    if state.competing_tags != CompetingTagTreatment::Blend {
        painter.circle_filled(personal.center(), 74.0 * scale, TAG_PERSONAL);
        let ring = if state.competing_tags == CompetingTagTreatment::Cutout {
            Color32::from_rgb(193, 112, 66)
        } else {
            Color32::from_rgb(222, 151, 91)
        };
        painter.circle_stroke(personal.center(), 74.0 * scale, scene.stroke(2.6, ring));
    }
    scene.label(448.0, 303.0, "tag/personal/fun", TAG_PERSONAL_TEXT);

    for (rect, title, path, accent) in [
        (cdint[0], "draft schema", "work/cdint", child_color),
        (cdint[1], "review sync", "work/cdint", child_color),
        (cdint[2], "write ingest", "work/cdint", child_color),
        (work[3], "release notes", "work/docs", TAG_WORK_FIELD),
        (work[4], "plan sprint", "work/ops", TAG_WORK_FIELD),
        (personal, "picnic plan", "personal/fun", TAG_PERSONAL),
    ] {
        scene.card(rect, title, false);
        painter.rect_filled(
            Rect::from_min_max(
                rect.left_top(),
                Pos2::new(rect.left() + 7.0 * scale, rect.bottom()),
            ),
            7.0 * scale,
            accent,
        );
        scene.label(
            rect.left(),
            rect.bottom() + 16.0 * scale,
            path,
            if path == "personal/fun" {
                TAG_PERSONAL_TEXT
            } else {
                TAG_WORK_TEXT
            },
        );
    }

    scene.label(
        26.0,
        556.0,
        &format!(
            "work links: {} · cdint links: {} · one personal/fun card deliberately crosses the work field",
            work_links.len(),
            child_links.len()
        ),
        FRAME,
    );
}

/// The second tag study deliberately has several branches beneath `work`.
/// That prevents a depth-only tint from being mistaken for a successful
/// hierarchy treatment when only one child path happens to be visible.
fn show_complex_tag_controls(ui: &mut egui::Ui, state: &mut LabState) {
    ui.label("Fixed complex fixture: full tag hierarchy by default. This study has no parent-rail alternative.");
    ui.horizontal_top(|ui| {
        ui.vertical(|ui| {
            ui.set_width(278.0);
            for (index, title) in TAG_STEPS.iter().enumerate() {
                let step = match index {
                    0 => TagStep::Scope,
                    1 => TagStep::HierarchyColor,
                    2 => TagStep::CompetingTags,
                    _ => TagStep::Connections,
                };
                ui.selectable_value(&mut state.complex_tag_step, step, *title);
            }
            ui.separator();
            match state.complex_tag_step {
                TagStep::Scope => {
                    ui.label("How much inherited hierarchy is visible?");
                    ui.add(
                        egui::Slider::new(&mut state.complex_tag_depth, 0..=5).custom_formatter(
                            |depth, _| if depth == 0.0 { "All".to_owned() } else { format!("{depth:.0} levels") },
                        ).text("Hierarchy depth"),
                    );
                    ui.small("All is the default. A positive value limits painted hierarchy layers from tag/work downward; task tags themselves do not change.");
                }
                TagStep::HierarchyColor => {
                    ui.label("How should sibling branches beneath work differ?");
                    ui.radio_value(&mut state.hierarchy_color, HierarchyColor::SameHue, "Same hue · hierarchy by shape");
                    ui.radio_value(&mut state.hierarchy_color, HierarchyColor::DepthShade, "Depth shade · shared branch family");
                    ui.radio_value(&mut state.hierarchy_color, HierarchyColor::BranchHue, "Branch hue · cdint/docs/ops distinct");
                    ui.small("The fixture shows work/cdint, work/docs, and work/ops together, then continues cdint through api, transport, and codec.");
                }
                TagStep::CompetingTags => {
                    ui.label("How does personal/fun remain legible inside work?");
                    ui.radio_value(&mut state.complex_competing_position, CompetingTagPosition::Blend, "Blend · ambiguous baseline");
                    ui.radio_value(&mut state.complex_competing_position, CompetingTagPosition::Foreground, "Foreground island");
                    ui.checkbox(&mut state.complex_cutout, "Cut out hierarchy fields behind it");
                    ui.checkbox(&mut state.complex_competing_outline, "Outline competing cloud");
                    ui.small("Cutout and outline are independent. The default is a foreground personal cloud with cutout on and outline off.");
                }
                TagStep::Connections => {
                    ui.label("How should same-tag members form neighbor tuples?");
                    ui.radio_value(&mut state.complex_neighbor_links, NeighborLinks::Nearest, "Nearest N neighbors");
                    ui.radio_value(&mut state.complex_neighbor_links, NeighborLinks::WithinRadius, "All within radius");
                    ui.add(
                        egui::Slider::new(&mut state.complex_neighbor_count, 0..=5)
                            .text("Nearby n-tuples"),
                    );
                    ui.add_enabled_ui(state.complex_neighbor_links == NeighborLinks::WithinRadius, |ui| {
                        ui.add(egui::Slider::new(&mut state.complex_neighbor_radius, 80.0..=600.0).text("Within radius"));
                    });
                    ui.checkbox(&mut state.complex_fill_local_gaps, "Fill gaps inside local cloud bodies");
                    ui.small("Default: five nearest neighbors. Zero leaves cloud members unconnected. “All within radius” ignores the N limit and joins every same-path pair in range.");
                }
            }
            ui.separator();
            ui.label("Current path");
            ui.small(format!(
                "depth {} · {} · {}{} · {}",
                if state.complex_tag_depth == 0 { "all".to_owned() } else { state.complex_tag_depth.to_string() },
                hierarchy_color_name(state.hierarchy_color),
                match state.complex_competing_position { CompetingTagPosition::Blend => "blend", CompetingTagPosition::Foreground => "foreground" },
                if state.complex_cutout { " + cutout" } else { "" },
                match state.complex_neighbor_links { NeighborLinks::Nearest => format!("{} nearest", state.complex_neighbor_count), NeighborLinks::WithinRadius => format!("all within {:.0}", state.complex_neighbor_radius) },
            ));
        });
        ui.separator();
        ui.vertical(|ui| {
            let width = ui.available_width().max(470.0);
            ui.heading("One parent · sibling tag branches · competing hierarchy");
            let size = Vec2::new(width, (width * 0.74).clamp(500.0, 720.0));
            let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
            draw_complex_tag_scene(&ui.painter_at(rect), rect, state);
            ui.small("All cloud bodies are behind cards. The cutout option makes the broad work field yield space to personal/fun while preserving its own cloud.");
        });
    });
}

fn hierarchy_color(mode: HierarchyColor, path: &str) -> (Color32, Color32) {
    let depth = path.matches('/').count();
    match mode {
        HierarchyColor::SameHue => (TAG_WORK_FIELD, TAG_WORK_TEXT),
        HierarchyColor::DepthShade => match depth {
            0 | 1 => (TAG_WORK_FIELD, TAG_WORK_TEXT),
            2 => (
                Color32::from_rgb(137, 192, 178),
                Color32::from_rgb(35, 99, 83),
            ),
            _ => (
                Color32::from_rgb(93, 157, 143),
                Color32::from_rgb(25, 76, 66),
            ),
        },
        HierarchyColor::BranchHue => match path {
            // Branch hue remains inside the work blue–teal neighborhood. The
            // variation encodes lineage without making siblings read like
            // unrelated categorical tags.
            "work/cdint" => (
                Color32::from_rgb(151, 204, 187),
                Color32::from_rgb(40, 105, 86),
            ),
            "work/cdint/api" => (
                Color32::from_rgb(113, 178, 146),
                Color32::from_rgb(28, 82, 65),
            ),
            "work/cdint/api/transport" => (
                Color32::from_rgb(94, 169, 163),
                Color32::from_rgb(24, 79, 75),
            ),
            "work/cdint/api/transport/codec" => (
                Color32::from_rgb(70, 144, 139),
                Color32::from_rgb(17, 61, 58),
            ),
            "work/cdint/api/transport/claude" => (
                Color32::from_rgb(105, 165, 197),
                Color32::from_rgb(34, 73, 105),
            ),
            "work/docs" => (
                Color32::from_rgb(155, 191, 214),
                Color32::from_rgb(45, 91, 125),
            ),
            "work/ops" => (
                Color32::from_rgb(133, 190, 203),
                Color32::from_rgb(31, 96, 110),
            ),
            _ => (TAG_WORK_FIELD, TAG_WORK_TEXT),
        },
    }
}

fn nearest_neighbor_links(centers: &[Pos2], neighbors: usize) -> Vec<(usize, usize)> {
    if neighbors == 0 || centers.len() < 2 {
        return Vec::new();
    }
    let mut links = BTreeSet::new();
    for (index, point) in centers.iter().enumerate() {
        let mut candidates = centers
            .iter()
            .enumerate()
            .filter(|(other, _)| *other != index)
            .map(|(other, candidate)| (point.distance(*candidate), other))
            .collect::<Vec<_>>();
        candidates.sort_by(|left, right| left.0.total_cmp(&right.0).then(left.1.cmp(&right.1)));
        for (_, other) in candidates.into_iter().take(neighbors) {
            links.insert((index.min(other), index.max(other)));
        }
    }
    links.into_iter().collect()
}

fn radius_neighbor_links(centers: &[Pos2], radius: f32) -> Vec<(usize, usize)> {
    let mut links = Vec::new();
    for (left, from) in centers.iter().enumerate() {
        for (right, to) in centers.iter().enumerate().skip(left + 1) {
            if from.distance(*to) <= radius {
                links.push((left, right));
            }
        }
    }
    links
}

fn draw_complex_tag_cloud(
    scene: &Scene<'_>,
    cards: &[Rect],
    fill: Color32,
    depth: usize,
    state: &LabState,
) {
    let centers = cards.iter().map(Rect::center).collect::<Vec<_>>();
    let links = match state.complex_neighbor_links {
        NeighborLinks::Nearest => nearest_neighbor_links(&centers, state.complex_neighbor_count),
        NeighborLinks::WithinRadius => {
            radius_neighbor_links(&centers, state.complex_neighbor_radius * scene.scale)
        }
    };
    let depth = u8::try_from(depth).map_or(f32::from(u8::MAX), f32::from);
    let radius = (72.0 - (depth * 8.0)).max(38.0) * scene.scale;
    if state.complex_fill_local_gaps && centers.len() >= 3 {
        // The convex fill removes small uncolored triangular gaps between
        // neighboring members. It is deliberately limited to a single tag
        // path, so unrelated sibling colors still push one another apart.
        scene.painter.add(egui::Shape::convex_polygon(
            centers.clone(),
            fill,
            Stroke::NONE,
        ));
    }
    draw_cloud_links(
        scene.painter,
        &centers,
        &links,
        radius * 1.15 / scene.scale,
        fill,
        scene.scale,
    );
    for card in cards {
        scene.painter.circle_filled(card.center(), radius, fill);
    }
}

fn draw_complex_tag_scene(painter: &egui::Painter, canvas: Rect, state: &LabState) {
    let scale = (canvas.width() / 900.0).min(canvas.height() / 700.0);
    let scene = Scene {
        painter,
        origin: canvas.min,
        scale,
    };
    painter.rect_filled(canvas, 8.0, CANVAS);

    let cdint = [
        scene.rect(100.0, 155.0, 136.0, 58.0),
        scene.rect(270.0, 108.0, 136.0, 58.0),
        scene.rect(270.0, 276.0, 136.0, 58.0),
    ];
    let api = scene.rect(430.0, 306.0, 138.0, 58.0);
    let transport = scene.rect(430.0, 411.0, 138.0, 58.0);
    let codec = scene.rect(430.0, 516.0, 138.0, 58.0);
    let claude = scene.rect(596.0, 516.0, 138.0, 58.0);
    let docs = [
        scene.rect(562.0, 128.0, 136.0, 58.0),
        scene.rect(702.0, 232.0, 136.0, 58.0),
    ];
    let ops = [
        scene.rect(108.0, 412.0, 136.0, 58.0),
        scene.rect(670.0, 438.0, 136.0, 58.0),
    ];
    let personal = scene.rect(435.0, 204.0, 150.0, 58.0);
    let work = [
        cdint[0], cdint[1], cdint[2], api, transport, codec, claude, docs[0], docs[1], ops[0],
        ops[1],
    ];
    let cdint_family = [cdint[0], cdint[1], cdint[2], api, transport, codec, claude];
    let api_family = [api, transport, codec, claude];
    let transport_family = [transport, codec, claude];
    let show_depth = |depth| state.complex_tag_depth == 0 || depth <= state.complex_tag_depth;

    if show_depth(1) {
        let (fill, text) = hierarchy_color(state.hierarchy_color, "work");
        draw_complex_tag_cloud(&scene, &work, fill, 1, state);
        scene.label(34.0, 48.0, "tag/work · ancestor context", text);
    }

    if show_depth(2) {
        let (fill, text) = hierarchy_color(state.hierarchy_color, "work/cdint");
        draw_complex_tag_cloud(&scene, &cdint_family, fill, 2, state);
        scene.label(78.0, 350.0, "work/cdint", text);

        let (fill, text) = hierarchy_color(state.hierarchy_color, "work/docs");
        draw_complex_tag_cloud(&scene, &docs, fill, 2, state);
        scene.label(690.0, 320.0, "work/docs", text);

        let (fill, text) = hierarchy_color(state.hierarchy_color, "work/ops");
        draw_complex_tag_cloud(&scene, &ops, fill, 2, state);
        scene.label(598.0, 505.0, "work/ops", text);
    }

    if show_depth(3) {
        let (fill, text) = hierarchy_color(state.hierarchy_color, "work/cdint/api");
        draw_complex_tag_cloud(&scene, &api_family, fill, 3, state);
        scene.label(410.0, 384.0, "work/cdint/api", text);
    }

    if show_depth(4) {
        let (fill, text) = hierarchy_color(state.hierarchy_color, "work/cdint/api/transport");
        draw_complex_tag_cloud(&scene, &transport_family, fill, 4, state);
        scene.label(410.0, 489.0, "work/cdint/api/transport", text);
    }

    if show_depth(5) {
        let (fill, text) = hierarchy_color(state.hierarchy_color, "work/cdint/api/transport/codec");
        draw_complex_tag_cloud(&scene, &[codec], fill, 5, state);
        scene.label(410.0, 594.0, "work/cdint/api/transport/codec", text);
        draw_complex_tag_cloud(&scene, &[claude], fill, 5, state);
        scene.label(580.0, 594.0, "work/cdint/api/transport/claude", text);
    }

    if state.complex_cutout {
        painter.circle_filled(personal.center(), 86.0 * scale, CANVAS);
    }

    if state.complex_competing_position == CompetingTagPosition::Foreground {
        painter.circle_filled(personal.center(), 72.0 * scale, TAG_PERSONAL);
    } else {
        painter.circle_filled(personal.center(), 68.0 * scale, TAG_PERSONAL);
    }
    if state.complex_competing_outline {
        painter.circle_stroke(
            personal.center(),
            72.0 * scale,
            scene.stroke(2.8, Color32::from_rgb(193, 112, 66)),
        );
    }

    for (rect, title, path, accent) in [
        (
            cdint[0],
            "parse exports",
            "work/cdint",
            hierarchy_color(state.hierarchy_color, "work/cdint").0,
        ),
        (
            cdint[1],
            "schema review",
            "work/cdint",
            hierarchy_color(state.hierarchy_color, "work/cdint").0,
        ),
        (
            cdint[2],
            "verify imports",
            "work/cdint",
            hierarchy_color(state.hierarchy_color, "work/cdint").0,
        ),
        (
            api,
            "api boundary",
            "work/cdint/api",
            hierarchy_color(state.hierarchy_color, "work/cdint/api").0,
        ),
        (
            transport,
            "transport seam",
            "work/cdint/api/transport",
            hierarchy_color(state.hierarchy_color, "work/cdint/api/transport").0,
        ),
        (
            codec,
            "codec limits",
            "work/cdint/api/transport/codec",
            hierarchy_color(state.hierarchy_color, "work/cdint/api/transport/codec").0,
        ),
        (
            claude,
            "claude review",
            "work/cdint/api/transport/claude",
            hierarchy_color(state.hierarchy_color, "work/cdint/api/transport/claude").0,
        ),
        (
            docs[0],
            "guide draft",
            "work/docs",
            hierarchy_color(state.hierarchy_color, "work/docs").0,
        ),
        (
            docs[1],
            "publish notes",
            "work/docs",
            hierarchy_color(state.hierarchy_color, "work/docs").0,
        ),
        (
            ops[0],
            "plan release",
            "work/ops",
            hierarchy_color(state.hierarchy_color, "work/ops").0,
        ),
        (
            ops[1],
            "run migration",
            "work/ops",
            hierarchy_color(state.hierarchy_color, "work/ops").0,
        ),
        (personal, "picnic plan", "personal/fun", TAG_PERSONAL),
    ] {
        scene.card(rect, title, false);
        painter.rect_filled(
            Rect::from_min_max(
                rect.left_top(),
                Pos2::new(rect.left() + 7.0 * scale, rect.bottom()),
            ),
            7.0 * scale,
            accent,
        );
        let text = if path == "personal/fun" {
            TAG_PERSONAL_TEXT
        } else {
            TAG_WORK_TEXT
        };
        scene.label(rect.left(), rect.bottom() + 15.0 * scale, path, text);
    }
    scene.label(
        28.0,
        682.0,
        &format!(
            "hierarchy depth: {} · {} · fill local gaps: {}",
            if state.complex_tag_depth == 0 {
                "all".to_owned()
            } else {
                state.complex_tag_depth.to_string()
            },
            match state.complex_neighbor_links {
                NeighborLinks::Nearest =>
                    format!("{} nearest neighbors", state.complex_neighbor_count),
                NeighborLinks::WithinRadius =>
                    format!("all within {:.0}", state.complex_neighbor_radius),
            },
            if state.complex_fill_local_gaps {
                "on"
            } else {
                "off"
            },
        ),
        FRAME,
    );
}

#[cfg(test)]
mod tests {
    use super::{
        CloudConnections, cloud_link_pairs, nearest_neighbor_links, proximity_clusters,
        radius_neighbor_links,
    };
    use eframe::egui::{Rect, pos2, vec2};

    #[test]
    fn proximity_is_transitive_and_does_not_join_remote_children() {
        let cards = [
            Rect::from_min_size(pos2(0.0, 0.0), vec2(10.0, 10.0)),
            Rect::from_min_size(pos2(15.0, 0.0), vec2(10.0, 10.0)),
            Rect::from_min_size(pos2(30.0, 0.0), vec2(10.0, 10.0)),
            Rect::from_min_size(pos2(100.0, 0.0), vec2(10.0, 10.0)),
        ];
        assert_eq!(
            proximity_clusters(&cards, 16.0),
            vec![vec![0, 1, 2], vec![3]]
        );
    }

    #[test]
    fn denser_link_choices_add_connections_without_changing_members() {
        let centers = [
            pos2(0.0, 0.0),
            pos2(80.0, 0.0),
            pos2(160.0, 0.0),
            pos2(240.0, 0.0),
            pos2(320.0, 0.0),
        ];
        assert_eq!(
            cloud_link_pairs(&centers, CloudConnections::MinimumTree).len(),
            4
        );
        assert!(
            cloud_link_pairs(&centers, CloudConnections::NearbyPairs).len()
                > cloud_link_pairs(&centers, CloudConnections::MinimumTree).len()
        );
        assert!(
            cloud_link_pairs(&centers, CloudConnections::DenseNeighborhood).len()
                > cloud_link_pairs(&centers, CloudConnections::NearbyPairs).len()
        );
    }

    #[test]
    fn nearby_neighbor_count_zero_leaves_a_hierarchy_path_unconnected() {
        let centers = [pos2(0.0, 0.0), pos2(80.0, 0.0), pos2(160.0, 0.0)];
        assert!(nearest_neighbor_links(&centers, 0).is_empty());
        assert_eq!(nearest_neighbor_links(&centers, 1), vec![(0, 1), (1, 2)]);
    }

    #[test]
    fn within_radius_links_every_pair_in_range() {
        let centers = [pos2(0.0, 0.0), pos2(50.0, 0.0), pos2(145.0, 0.0)];
        assert_eq!(radius_neighbor_links(&centers, 60.0), vec![(0, 1)]);
        assert_eq!(
            radius_neighbor_links(&centers, 150.0),
            vec![(0, 1), (0, 2), (1, 2)]
        );
    }
}

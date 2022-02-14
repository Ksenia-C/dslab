use druid::piet::{FontFamily, Text, TextLayout, TextLayoutBuilder};
use druid::widget::prelude::*;
use druid::Color;
use druid::Point;

use crate::app_data::AppData;

pub fn paint_colored_text(
    ctx: &mut PaintCtx,
    text: &str,
    font: f64,
    mut pos: Point,
    centerx: bool,
    centery: bool,
    color: Color,
) {
    let layout = ctx
        .text()
        .new_text_layout(text.to_string())
        .font(FontFamily::SYSTEM_UI, font)
        .text_color(color)
        .build()
        .unwrap();
    let text_size = layout.size();
    if centerx {
        pos.x -= text_size.width / 2.;
    }
    if centery {
        pos.y -= text_size.height / 2.;
    }
    ctx.draw_text(&layout, pos);
}

pub fn paint_text(ctx: &mut PaintCtx, text: &str, font: f64, pos: Point, centerx: bool, centery: bool) {
    paint_colored_text(ctx, text, font, pos, centerx, centery, Color::WHITE);
}

pub fn draw_arrow(ctx: &mut PaintCtx, pos: Point, is_download: bool) {
    let w1 = 4.;
    let w2 = 7.;
    let mut h1 = -7.;
    let h0 = 0.;
    let mut h2 = 7.;
    if !is_download {
        std::mem::swap(&mut h1, &mut h2);
    }
    let mut arrow = vec![
        Point::new(w1, h0),
        Point::new(w1, h1),
        Point::new(-w1, h1),
        Point::new(-w1, h0),
        Point::new(-w2, h0),
        Point::new(0., h2),
        Point::new(w2, h0),
    ];
    for point in arrow.iter_mut() {
        point.x += pos.x;
        point.y += pos.y;
    }
    let poly = crate::poly::Poly::from_vec(arrow);
    ctx.fill(
        poly,
        &(if is_download {
            Color::rgb8(0, 200, 0)
        } else {
            Color::rgb8(0, 210, 210)
        }),
    );
}

pub fn draw_download(ctx: &mut PaintCtx, pos: Point) {
    draw_arrow(ctx, pos, true);
}

pub fn draw_upload(ctx: &mut PaintCtx, pos: Point) {
    draw_arrow(ctx, pos, false);
}

pub fn get_text_task_info(data: &AppData, task_id: usize) -> String {
    let task_info = data.task_info.borrow()[task_id].as_ref().unwrap().clone();
    let task = &data.graph.borrow().tasks[task_id];

    let mut result = String::new();
    result += &format!("Task: {}\n\n", task.name);
    result += &format!("Total time: {:.3}\n\n", task_info.completed - task_info.scheduled);
    let mut inputs: Vec<String> = Vec::new();
    let mut outputs: Vec<String> = Vec::new();
    for transfer in data.transfers.borrow().iter() {
        if transfer.task != task_id {
            continue;
        }
        if transfer.end <= task_info.started {
            inputs.push(transfer.name.clone());
        } else {
            outputs.push(transfer.name.clone());
        }
    }
    result += &format!("Inputs: {}\n\n", inputs.join(", "));
    result += &format!("Outputs: {}\n\n", outputs.join(", "));
    result
}

use std::sync::{Arc, Mutex};

use iced::{Color, Element, Length, Point, Rectangle, Size, Vector};
use iced::canvas::{Cache, Cursor, Geometry, LineCap, Path, Program, Stroke, Text};
use iced_native::{Renderer, Widget, layout, renderer};

use vmouse::{Axis, AXIS_LIN, AXIS_ROT};

use crate::message::Message;


/// CurveGraph displays an axis map and value
#[derive(Debug)]
pub struct CurveGraph {
    pub a: Axis,
    i: Arc<Mutex<CurveGraphInner>>,
}

#[derive(Debug)]
struct CurveGraphInner {
    s: f32,
    v: f32,
    c: Cache,
}

const N: isize = 100;

impl CurveGraph {
    pub fn new(a: Axis, s: f32, v: f32) -> Self {
        Self {
            a,
            i: Arc::new(Mutex::new(CurveGraphInner{
                s, v, c: Cache::new(),
            })),
        }
    }

    pub fn set_scale(&self, s: f32) {
        let mut i = self.i.lock().unwrap();
        if i.s != s {
            i.c.clear();
        }
        i.s = s;
    }

    pub fn set_value(&self, v: f32) {
        let mut i = self.i.lock().unwrap();
        if i.v != v {
            i.c.clear();
        }
        i.v = v;
    }
}

const BOUNDS: f32 = 10.0;

impl Program<Message> for Arc<CurveGraph> {
    fn draw(&self, bounds: Rectangle, _cursor: Cursor) -> Vec<Geometry> {
        let inner = self.i.lock().unwrap();

        let g = inner.c.draw(bounds.size(), |f| {

            let center = f.center();
            let b = bounds.size();

            let bx = bounds.size().width / 2.0 - BOUNDS;
            let by = bounds.size().height / 2.0 - BOUNDS;

            // Setup stroke type
            let mut thin_stroke = Stroke {
                width: 2.0,
                color: Color::BLACK,
                line_cap: LineCap::Round,
                ..Stroke::default()
            };

            // Bounding box
            let p = Path::rectangle(Point::new(1.0, 1.0), Size::new(b.width-2.0, b.height-2.0));
            f.stroke(&p, thin_stroke);

            // Title
            let t = Text{
                content: self.a.to_string(),
                position: Point::new(10.0, 10.0),
                size: 25.0,
                ..Default::default()
            };

            f.fill_text(t);

            // Axes

            thin_stroke.color = Color::from_rgb8(0xDC, 0xDC, 0xDC);
            let p = Path::line(Point{x: bx, y: 0.0}, Point{x: -bx, y: 0.0});
            f.with_save(|f| {
                f.translate(Vector::new(center.x, center.y));
                f.stroke(&p, thin_stroke);
            });

            let p = Path::line(Point{x: 0.0, y: -by}, Point{x: 0.0, y: by});
            f.with_save(|f| {
                f.translate(Vector::new(center.x, center.y));
                f.stroke(&p, thin_stroke);
            });

            thin_stroke.color = Color::BLACK;

            let p = Path::new(|b| {
                let mut last = Point{ x: -bx, y: -by };

                for i in -N..N+1 {
                    let x = i as f32 / N as f32;
                    let y = inner.s * x.powi(3) + (1.0 - inner.s) * x;

                    let p = Point{ x: x * bx, y: y * -by };

                    b.quadratic_curve_to(last, p);


                    //println!("x: {:?} y: {:?}", x, y);
                    //println!("prev: {:?} next: {:?}", last, next);

                    last = p;
                }
            });

            f.with_save(|f| {
                f.translate(Vector::new(center.x, center.y));
                f.stroke(&p, thin_stroke);
            });



            // Center marker
            let y = inner.s * inner.v.powi(3) + (1.0 - inner.s) * inner.v;
            let p = Point{ x: inner.v * bx, y: y * -by};
            let circle = Path::circle(p, 5.0);

            f.with_save(|f| {
                f.translate(Vector::new(center.x, center.y));
                f.fill(&circle, Color::from_rgb8(0x12, 0x93, 0xD8));
            });
        });

        //  Return geometry
        vec![g]
    }
}


impl <M, R> Widget<M, R> for CurveGraph 
where
    R: Renderer,
{
    fn width(&self) -> Length {
        Length::Fill
    }

    fn height(&self) -> Length {
        Length::Fill
    }

    fn layout(
        &self,
        _renderer: &R,
        limits: &layout::Limits,
    ) -> layout::Node {
        layout::Node::new(limits.fill())
    }

    fn draw(
        &self,
        renderer: &mut R,
        _style: &renderer::Style,
        layout: iced_native::Layout<'_>,
        _cursor_position: iced::Point,
        _viewport: &iced::Rectangle,
    ) {
        renderer.fill_quad(
            renderer::Quad {
                bounds: layout.bounds(),
                border_radius: 1.0,
                border_width: 1.0,
                border_color: Color::BLACK,
            },
            Color::WHITE,
        )
    }

    fn hash_layout(&self, state: &mut iced_native::Hasher) {
        use std::hash::Hash;

        let i = self.i.lock().unwrap();

        self.a.hash(state);
        i.v.to_bits().hash(state);
        i.s.to_bits().hash(state);
    }
}

impl<'a, M> Into<Element<'a, M>> for CurveGraph{
    fn into(self) -> Element<'a, M> {
        Element::new(self)
    }
}

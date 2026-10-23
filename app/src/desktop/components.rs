//! Iced buttons do not participate in keyboard focus; keep that in the shared component.
use super::design;
use iced::{
    advanced::{
        layout, mouse, renderer,
        widget::{
            operation::{Focusable, Operation},
            tree, Tree,
        },
        Clipboard, Layout, Shell, Widget,
    },
    keyboard, widget, Element, Event, Length, Padding, Rectangle, Renderer, Size, Theme,
};

pub struct Button<'a, Message> {
    inner: widget::Button<'a, Message>,
    message: Option<Message>,
}
#[derive(Default)]
struct Focus(bool);
impl Focusable for Focus {
    fn is_focused(&self) -> bool {
        self.0
    }
    fn focus(&mut self) {
        self.0 = true;
    }
    fn unfocus(&mut self) {
        self.0 = false;
    }
}
pub fn button<'a, Message: Clone + 'a>(
    content: impl Into<Element<'a, Message>>,
) -> Button<'a, Message> {
    Button {
        inner: widget::button(content)
            .style(design::secondary)
            .padding([8, 10])
            .height(design::CONTROL),
        message: None,
    }
}
impl<'a, Message: Clone + 'a> Button<'a, Message> {
    pub fn on_press(self, message: Message) -> Self {
        self.on_press_maybe(Some(message))
    }
    pub fn on_press_maybe(mut self, message: Option<Message>) -> Self {
        self.inner = self.inner.on_press_maybe(message.clone());
        self.message = message;
        self
    }
    pub fn width(mut self, width: impl Into<Length>) -> Self {
        self.inner = self.inner.width(width);
        self
    }
    pub fn height(mut self, height: impl Into<Length>) -> Self {
        self.inner = self.inner.height(height);
        self
    }
    pub fn padding(mut self, padding: impl Into<Padding>) -> Self {
        self.inner = self.inner.padding(padding);
        self
    }
    pub fn style(
        mut self,
        style: impl Fn(&Theme, widget::button::Status) -> widget::button::Style + 'a,
    ) -> Self {
        self.inner = self.inner.style(style);
        self
    }
}
impl<'a, Message: Clone + 'a> Widget<Message, Theme, Renderer> for Button<'a, Message> {
    fn size(&self) -> Size<Length> {
        self.inner.size()
    }
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<Focus>()
    }
    fn state(&self) -> tree::State {
        tree::State::new(Focus::default())
    }
    fn children(&self) -> Vec<Tree> {
        vec![Tree::new(
            &self.inner as &dyn Widget<Message, Theme, Renderer>,
        )]
    }
    fn diff(&self, tree: &mut Tree) {
        tree.diff_children(&[&self.inner as &dyn Widget<Message, Theme, Renderer>]);
    }
    fn layout(
        &mut self,
        tree: &mut Tree,
        renderer: &Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        self.inner.layout(
            &mut tree.children[0],
            renderer,
            &limits
                .min_width(design::DENSE_TARGET)
                .min_height(design::DENSE_TARGET),
        )
    }
    fn operate(
        &mut self,
        tree: &mut Tree,
        layout: Layout<'_>,
        _: &Renderer,
        operation: &mut dyn Operation,
    ) {
        if self.message.is_some() {
            operation.focusable(None, layout.bounds(), tree.state.downcast_mut::<Focus>());
        }
    }
    fn update(
        &mut self,
        tree: &mut Tree,
        event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &Renderer,
        clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
        viewport: &Rectangle,
    ) {
        if tree.state.downcast_ref::<Focus>().0
            && matches!(
                event,
                Event::Keyboard(keyboard::Event::KeyPressed {
                    key: keyboard::Key::Named(
                        keyboard::key::Named::Enter | keyboard::key::Named::Space
                    ),
                    ..
                })
            )
        {
            if let Some(message) = &self.message {
                shell.publish(message.clone());
                shell.capture_event();
                return;
            }
        }
        self.inner.update(
            &mut tree.children[0],
            event,
            layout,
            cursor,
            renderer,
            clipboard,
            shell,
            viewport,
        );
    }
    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut Renderer,
        theme: &Theme,
        style: &renderer::Style,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        self.inner.draw(
            &tree.children[0],
            renderer,
            theme,
            style,
            layout,
            cursor,
            viewport,
        );
        if tree.state.downcast_ref::<Focus>().0 {
            use iced::advanced::Renderer as _;
            renderer.fill_quad(
                renderer::Quad {
                    bounds: layout.bounds(),
                    border: iced::Border {
                        width: 2.0,
                        color: design::Palette::new(theme.palette().background.r < 0.5).accent_text,
                        radius: 8.0.into(),
                    },
                    ..Default::default()
                },
                iced::Color::TRANSPARENT,
            );
        }
    }
    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
        renderer: &Renderer,
    ) -> mouse::Interaction {
        self.inner
            .mouse_interaction(&tree.children[0], layout, cursor, viewport, renderer)
    }
}
impl<'a, Message: Clone + 'a> From<Button<'a, Message>> for Element<'a, Message> {
    fn from(button: Button<'a, Message>) -> Self {
        Element::new(button)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn keyboard_buttons_activate_only_when_enabled_and_focused() {
        use iced::advanced::widget::operation;
        let renderer = Renderer::new(design::UI, 13.0.into());
        for enabled in [true, false] {
            let mut button = button(widget::text("Tag")).on_press_maybe(enabled.then_some(7));
            let mut tree = Tree::new(&button as &dyn Widget<i32, Theme, Renderer>);
            let node = button.layout(
                &mut tree,
                &renderer,
                &layout::Limits::new(Size::ZERO, Size::new(200.0, 40.0)),
            );
            let mut focus = operation::focusable::focus_next::<()>();
            // Focus next first counts, then returns a focus operation.
            button.operate(&mut tree, Layout::new(&node), &renderer, &mut focus);
            if let operation::Outcome::Chain(mut next) = focus.finish() {
                button.operate(&mut tree, Layout::new(&node), &renderer, next.as_mut());
            }
            let mut messages = Vec::new();
            let event = Event::Keyboard(keyboard::Event::KeyPressed {
                key: keyboard::Key::Named(keyboard::key::Named::Enter),
                modified_key: keyboard::Key::Named(keyboard::key::Named::Enter),
                physical_key: keyboard::key::Physical::Unidentified(
                    keyboard::key::NativeCode::Unidentified,
                ),
                location: keyboard::Location::Standard,
                modifiers: keyboard::Modifiers::empty(),
                text: None,
                repeat: false,
            });
            button.update(
                &mut tree,
                &event,
                Layout::new(&node),
                mouse::Cursor::Unavailable,
                &renderer,
                &mut iced::advanced::clipboard::Null,
                &mut Shell::new(&mut messages),
                &Rectangle::with_size(Size::new(200.0, 40.0)),
            );
            assert_eq!(messages, if enabled { vec![7] } else { vec![] });
        }
    }
}

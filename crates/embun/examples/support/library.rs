//! A small photo library: the app the `gallery` and `photos` examples show.

use embun::*;

use super::photos::landscape;

pub struct Library {
    pub photos: Vec<Image>,
    pub album: usize,
    pub favorites: Vec<bool>,
    pub selected: Option<usize>,
    pub query: String,
}

#[derive(Clone, Debug)]
pub enum Msg {
    Album(usize),
    Select(usize),
    Favorite,
    Share,
    Search(String),
}

pub const ALBUMS: [&str; 4] = ["All Photos", "Favorites", "Travel", "Family"];

impl Library {
    pub fn new() -> Self {
        let photos: Vec<Image> = (0..12).map(|i| landscape(i, 320, 240)).collect();
        let favorites = vec![false; photos.len()];
        Self { photos, album: 0, favorites, selected: Some(4), query: String::new() }
    }
}

impl App for Library {
    type Message = Msg;

    fn update(&mut self, message: Msg) {
        match message {
            Msg::Album(i) => self.album = i,
            Msg::Select(i) => self.selected = Some(i),
            Msg::Favorite => {
                if let Some(i) = self.selected {
                    self.favorites[i] = !self.favorites[i];
                }
            }
            Msg::Share => {}
            Msg::Search(query) => self.query = query,
        }
    }

    fn view(&self, theme: &Theme) -> Element<Msg> {
        let visible: Vec<usize> = (0..self.photos.len()).filter(|&i| self.album != 1 || self.favorites[i]).collect();
        let tiles = visible.iter().map(|&i| {
            let selected = self.selected == Some(i);
            let tile = div()
                .key(format!("photo{i}"))
                .w(200.0)
                .h(150.0)
                .rounded(10.0)
                .child(image(self.photos[i].clone()).w(200.0).h(150.0).rounded(10.0))
                .on_press(Msg::Select(i))
                .label(format!("Photo {}", i + 1));
            if selected { tile.border(3.0, theme.accent()) } else { tile }
        });
        let grid = row().flex_wrap().gap_3().pt_16().pb_6().pl(252.0).pr_6().children(tiles);

        let sidebar = sidebar()
            .absolute()
            .top(8.0)
            .left(8.0)
            .bottom(8.0)
            .child(sidebar_heading("Library").text_color(theme.secondary_label()))
            .children(ALBUMS.iter().enumerate().map(|(i, name)| sidebar_item(*name, self.album == i, Msg::Album(i)).key(format!("album{i}"))));

        let favorite = self.selected.is_some_and(|i| self.favorites[i]);
        let toolbar = row()
            .absolute()
            .top(10.0)
            .left(244.0)
            .right(12.0)
            .items_center()
            .gap_3()
            .child(
                toolbar_group()
                    .child(toolbar_item("‹", Msg::Album(0)).label("Back"))
                    .child(toolbar_item("›", Msg::Album(0)).label("Forward")),
            )
            .child(text(ALBUMS[self.album]).text_size(15.0).font_semibold())
            .child(spacer())
            .child(
                toolbar_group()
                    .px_1()
                    .child(text_input(self.query.clone(), Msg::Search).placeholder("Search").w(180.0).rounded_full().key("search").label("Search")),
            )
            .child(toolbar_group().child(toolbar_item(if favorite { "♥" } else { "♡" }, Msg::Favorite).key("favorite").label("Favorite")))
            .child(button("Share").size(ControlSize::Large).prominent().on_press(Msg::Share).key("share"));

        div().child(column().scroll_y().h_full().child(grid)).child(sidebar).child(toolbar)
    }
}

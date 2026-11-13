fn main() {
    topcoat::tailwind::BuildConfig::new()
        .input("styles.css")
        .render()
        .unwrap();

    topcoat::icon::iconify::BuildConfig::new()
        .cache_dir("icons")
        .icon_set("si")
        .stage()
        .unwrap();
}

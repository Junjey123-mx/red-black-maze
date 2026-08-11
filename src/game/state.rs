/// Estados de alto nivel de la aplicación.
///
/// Por ahora la aplicación arranca directamente en `Playing`;
/// los demás estados son vocabulario arquitectónico para
/// tareas futuras.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum GameState {
    Welcome,
    LevelSelect,
    Playing,
    Victory,
}

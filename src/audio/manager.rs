use std::path::Path;

use raylib::prelude::*;

/// Única ubicación de la ruta del archivo de música de fondo.
const BACKGROUND_MUSIC_PATH: &str = "assets/audio/music/background.ogg";

/// Gestor centralizado de la música de fondo de la aplicación.
///
/// `Music<'aud>` está atada por vida a la `RaylibAudio` que la cargó
/// (impuesto por el propio `raylib-rs`), por lo que este manager toma
/// prestada esa `RaylibAudio` en lugar de poseerla: la posesión del
/// dispositivo de audio vive en `run()`, y este struct únicamente
/// vive tan tiempo como esa referencia. No hay auto-referencia, ni
/// `unsafe`, ni `transmute`: es el modelo de propiedad más simple que
/// el API segura de `raylib-rs` permite.
///
/// La ausencia de pista (archivo faltante, fallo de decodificación,
/// o dispositivo de audio no disponible) se representa con
/// `music: None`; todas las operaciones son no-op seguras en ese caso.
pub(crate) struct AudioManager<'aud> {
    music: Option<Music<'aud>>,
}

impl<'aud> AudioManager<'aud> {
    /// Construye el manager e intenta cargar la música de fondo
    /// EXACTAMENTE una vez. Si la carga tiene éxito, la reproducción
    /// comienza de inmediato (sin requerir entrada de teclado).
    ///
    /// `audio` es `None` cuando el dispositivo de audio no pudo
    /// inicializarse; en ese caso el manager queda deshabilitado.
    pub(crate) fn new(audio: Option<&'aud RaylibAudio>) -> Self {
        let music = audio.and_then(Self::load_background_music);

        if let Some(music) = &music {
            music.play_stream();
        }

        Self { music }
    }

    fn load_background_music(audio: &'aud RaylibAudio) -> Option<Music<'aud>> {
        if !Path::new(BACKGROUND_MUSIC_PATH).exists() {
            eprintln!(
                "Música de fondo no encontrada en '{BACKGROUND_MUSIC_PATH}'; continuando sin música."
            );

            return None;
        }

        match audio.new_music(BACKGROUND_MUSIC_PATH) {
            Ok(mut music) => {
                music.set_looping(true);

                Some(music)
            }

            Err(error) => {
                eprintln!("Error al cargar la música de fondo: {error}");

                None
            }
        }
    }

    /// Debe llamarse exactamente una vez por iteración del bucle
    /// principal mientras haya una pista cargada, independientemente
    /// del `GameState` activo. No-op seguro si no hay pista.
    pub(crate) fn update(&self) {
        if let Some(music) = &self.music {
            music.update_stream();
        }
    }

    /// Inicia o reanuda la reproducción usando el recurso ya
    /// cargado (nunca recarga desde disco). No-op seguro si no hay
    /// pista.
    pub(crate) fn play_music(&self) {
        if let Some(music) = &self.music {
            music.resume_stream();
        }
    }

    /// Pausa la reproducción. No-op seguro si no hay pista.
    pub(crate) fn pause_music(&self) {
        if let Some(music) = &self.music {
            music.pause_stream();
        }
    }

    /// Detiene la reproducción. No-op seguro si no hay pista.
    pub(crate) fn stop_music(&self) {
        if let Some(music) = &self.music {
            music.stop_stream();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /*
     * Prueba pura, sin dispositivo de audio ni archivo local: solo
     * verifica que la ruta genérica del proyecto es la esperada.
     */
    #[test]
    fn background_music_path_is_the_expected_generic_project_path() {
        assert_eq!(BACKGROUND_MUSIC_PATH, "assets/audio/music/background.ogg");
    }
}

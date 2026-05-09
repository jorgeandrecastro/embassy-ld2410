// Copyright (C) 2026 Jorge Andre Castro
// GPL-2.0-or-later

//! # embassy-ld2410
//!
//! Driver `no_std` asynchrone pour le radar de présence humaine **LD2410C** (24GHz FMCW).
//! Conçu pour l'écosystème [Embassy](https://embassy.dev) sur RP2040 et RP2350.
//!
//! ## Caractéristiques
//!
//! - Entièrement asynchrone (`async/await`)
//! - Zéro allocation dynamique (`no_std`, `no_alloc`)
//! - Synchronisation automatique sur le header de trame
//! - Support optionnel de `defmt` pour le logging embarqué
//! - Compatible RP2040, RP2350-A et RP2350-B
//!
//! ## Format de trame LD2410C
//!
//! ```text
//! Offset  Taille  Description
//! ------  ------  -----------
//! 0..4    4       Header  : F4 F3 F2 F1
//! 4..6    2       Longueur: 0D 00 (13 bytes)
//! 6       1       Type    : 02
//! 7       1       Marqueur: AA
//! 8       1       État cible (0=aucun, 1=mouvement, 2=statique, 3=combiné)
//! 9..11   2       Distance mouvement (cm, little-endian)
//! 11      1       Énergie mouvement (0-100)
//! 12..14  2       Distance statique (cm, little-endian)
//! 14      1       Énergie statique (0-100)
//! 15..17  2       Distance détection (cm, little-endian)
//! 17      1       Tail : 55
//! 18      1       Checksum
//! 19..23  4       Footer : F8 F7 F6 F5
//! ```
//!
//! ## Câblage
//!
//! ```text
//! LD2410C TX  →  Pico GP1 (RX)
//! LD2410C RX  →  Pico GP0 (TX)
//! LD2410C VCC →  Pico VBUS (5V)
//! LD2410C GND →  Pico GND
//! ```
//!
//! ## Exemple d'utilisation
//!
//! ```rust,no_run
//! use embassy_rp::bind_interrupts;
//! use embassy_rp::uart::{BufferedUart, BufferedInterruptHandler, Config as UartConfig};
//! use embassy_rp::peripherals::UART0;
//! use embassy_ld2410::{Ld2410, TargetState};
//! use static_cell::StaticCell;
//!
//! bind_interrupts!(struct Irqs {
//!     UART0_IRQ => BufferedInterruptHandler<UART0>;
//! });
//!
//! static TX_BUF: StaticCell<[u8; 16]>  = StaticCell::new();
//! static RX_BUF: StaticCell<[u8; 256]> = StaticCell::new();
//!
//! # async fn example(p: embassy_rp::Peripherals) {
//! let mut uart_config = UartConfig::default();
//! uart_config.baudrate = 256_000;
//!
//! let uart = BufferedUart::new(
//!     p.UART0,
//!     p.PIN_0,  // TX
//!     p.PIN_1,  // RX
//!     Irqs,
//!     TX_BUF.init([0u8; 16]),
//!     RX_BUF.init([0u8; 256]),
//!     uart_config,
//! );
//! let (_tx, rx) = uart.split();
//! let mut radar = Ld2410::new(rx);
//!
//! loop {
//!     match radar.read_presence().await {
//!         Ok(data) => {
//!             match data.target_state {
//!                 Some(TargetState::Moving)   => { /* cible en mouvement */ }
//!                 Some(TargetState::Static)   => { /* cible immobile */ }
//!                 Some(TargetState::Combined) => { /* les deux */ }
//!                 _                           => { /* aucune présence */ }
//!             }
//!         }
//!         Err(_) => { /* erreur UART, réessaie automatiquement */ }
//!     }
//! }
//! # }
//! ```

#![no_std]
#![forbid(unsafe_code)]

pub mod error;
use error::Ld2410Error;
use embassy_rp::uart::BufferedUartRx;
use embedded_io_async::Read;

/// État de détection retourné par le radar LD2410C.
///
/// Le LD2410C distingue les cibles en mouvement actif des cibles
/// statiques (micro-mouvements de respiration).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum TargetState {
    /// Aucune présence détectée dans la zone de couverture.
    None     = 0x00,
    /// Cible en mouvement actif détectée.
    Moving   = 0x01,
    /// Cible statique (immobile, respiration) détectée.
    Static   = 0x02,
    /// Cibles statiques et en mouvement simultanément détectées.
    Combined = 0x03,
}

/// Données de présence décodées depuis une trame LD2410C.
///
/// Toutes les distances sont exprimées en centimètres.
/// Les valeurs d'énergie sont comprises entre 0 et 100.
#[derive(Debug, Default, Clone, Copy)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct PresenceData {
    /// État actuel de la cible détectée.
    pub target_state: Option<TargetState>,

    /// Distance de la cible en mouvement (cm).
    pub moving_distance: u16,

    /// Énergie du signal de mouvement (0–100).
    pub moving_energy: u8,

    /// Distance de la cible statique (cm).
    pub static_distance: u16,

    /// Énergie du signal statique (0–100).
    pub static_energy: u8,

    /// Distance de détection globale calculée par le module (cm).
    pub detection_distance: u16,
}

/// Driver asynchrone pour le radar de présence humaine LD2410C.
///
/// Utilise un [`BufferedUartRx`] pour recevoir les trames UART
/// sans bloquer l'exécuteur Embassy.
///
/// # Création
///
/// Construire via [`Ld2410::new`] en passant le canal RX d'un
/// [`BufferedUart`](embassy_rp::uart::BufferedUart) configuré à **256 000 bauds**.
pub struct Ld2410 {
    rx: BufferedUartRx,
}

impl Ld2410 {
    /// Crée un nouveau driver LD2410C.
    ///
    /// # Paramètres
    ///
    /// - `rx` : canal RX d'un `BufferedUart` configuré à 256 000 bauds.
    ///
    /// # Exemple
    ///
    /// ```rust,no_run
    /// # use embassy_rp::uart::BufferedUartRx;
    /// # use embassy_ld2410::Ld2410;
    /// # fn example(rx: BufferedUartRx) {
    /// let mut radar = Ld2410::new(rx);
    /// # }
    /// ```
    pub fn new(rx: BufferedUartRx) -> Self {
        Self { rx }
    }

    /// Lit et décode la prochaine trame de présence valide.
    ///
    /// Cette méthode se synchronise automatiquement sur le header
    /// `F4 F3 F2 F1` et réessaie silencieusement en cas de trame
    /// corrompue. Elle ne retourne une erreur que si l'UART échoue.
    ///
    /// # Retour
    ///
    /// - `Ok(PresenceData)` : trame valide décodée.
    /// - `Err(Ld2410Error::UartError)` : erreur de lecture UART.
    ///
    /// # Exemple
    ///
    /// ```rust,no_run
    /// # use embassy_ld2410::{Ld2410, TargetState};
    /// # async fn example(mut radar: Ld2410) {
    /// match radar.read_presence().await {
    ///     Ok(data) => {
    ///         if data.target_state == Some(TargetState::Moving) {
    ///             // présence en mouvement détectée
    ///         }
    ///     }
    ///     Err(_) => { /* erreur UART */ }
    /// }
    /// # }
    /// ```
    pub async fn read_presence(&mut self) -> Result<PresenceData, Ld2410Error> {
        let mut b = [0u8; 1];

        loop {
            // Synchronisation byte par byte sur le header F4 F3 F2 F1
            loop {
                self.rx.read_exact(&mut b).await.map_err(|_| Ld2410Error::UartError)?;
                if b[0] != 0xF4 { continue; }
                self.rx.read_exact(&mut b).await.map_err(|_| Ld2410Error::UartError)?;
                if b[0] != 0xF3 { continue; }
                self.rx.read_exact(&mut b).await.map_err(|_| Ld2410Error::UartError)?;
                if b[0] != 0xF2 { continue; }
                self.rx.read_exact(&mut b).await.map_err(|_| Ld2410Error::UartError)?;
                if b[0] == 0xF1 { break; }
            }

            // Lecture des 19 bytes de payload un par un
            let mut buf = [0u8; 19];
            for i in 0..19 {
                self.rx.read_exact(&mut b).await.map_err(|_| Ld2410Error::UartError)?;
                buf[i] = b[0];
            }

            // Validation du marqueur de données (0xAA à offset 3)
            if buf[3] != 0xAA { continue; }

            // Validation du footer F8 F7 F6 F5 (offset 15..19)
            if &buf[15..19] != &[0xF8, 0xF7, 0xF6, 0xF5] { continue; }

            let state = match buf[4] {
                0x01 => Some(TargetState::Moving),
                0x02 => Some(TargetState::Static),
                0x03 => Some(TargetState::Combined),
                _    => None,
            };

            return Ok(PresenceData {
                target_state:       state,
                moving_distance:    u16::from_le_bytes([buf[5],  buf[6]]),
                moving_energy:      buf[7],
                static_distance:    u16::from_le_bytes([buf[8],  buf[9]]),
                static_energy:      buf[10],
                detection_distance: u16::from_le_bytes([buf[11], buf[12]]),
            });
        }
    }
}
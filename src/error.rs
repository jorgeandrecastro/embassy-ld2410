// Copyright (C) 2026 Jorge Andre Castro
// GPL-2.0-or-later

//! Erreurs retournées par le driver [`embassy-ld2410`](crate).

/// Erreurs possibles lors de la communication avec le LD2410C.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum Ld2410Error {
    /// Erreur de lecture sur le bus UART (timeout, overrun, etc.).
    UartError,

    /// Header de trame invalide attendu : `0xF4 0xF3 0xF2 0xF1`.
    InvalidHeader,

    /// Footer de trame invalide attendu : `0xF8 0xF7 0xF6 0xF5`.
    InvalidFooter,

    /// Trame reçue incomplète ou corrompue.
    IncompleteFrame,
}
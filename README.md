# 🛰️ embassy-ld2410

[![Crates.io](https://img.shields.io/crates/v/embassy-ld2410.svg)](https://crates.io/crates/embassy-ld2410)
[![Docs.rs](https://docs.rs/embassy-ld2410/badge.svg)](https://docs.rs/embassy-ld2410)
[![License: GPL-2.0-or-later](https://img.shields.io/badge/License-GPL--2.0--or--later-blue.svg)](https://www.gnu.org/licenses/gpl-2.0-or-later)
[![Platform: RP2040/RP2350](https://img.shields.io/badge/Platform-RP2040%20%7C%20RP2350-orange.svg)](https://www.raspberrypi.com/)
[![Embedded: Rust](https://img.shields.io/badge/Embedded-Rust-black?logo=rust)](https://www.rust-lang.org/)

Driver `no_std` asynchrone pour le radar de présence humaine **LD2410C** (24GHz FMCW),
conçu pour l'écosystème [Embassy](https://embassy.dev) sur RP2040 et RP2350.

---

## 📖 Description

Le **LD2410C** est un module radar haute sensibilité qui surpasse les capteurs PIR
traditionnels. Il détecte les **mouvements actifs** ainsi que les **micro-mouvements**
(respiration) d'une personne totalement immobile, jusqu'à 6 mètres de distance.

Ce driver parse le protocole série binaire du LD2410C avec synchronisation automatique
sur le header de trame, rendant l'intégration robuste même en cas de démarrage
au milieu d'une trame.

---

## ✨ Caractéristiques

- ✅ Entièrement asynchrone (`async/await`, Embassy)
- ✅ Zéro allocation (`no_std`, `no_alloc`)
- ✅ Synchronisation automatique sur header de trame
- ✅ Résilience aux trames corrompues (réessai silencieux)
- ✅ Support optionnel de `defmt`
- ✅ Compatible RP2040, RP2350-A, RP2350-B

---

## 🛠️ Spécifications Matérielles

| Paramètre        | Valeur                        |
| :--------------- | :---------------------------- |
| Interface        | UART                          |
| Baudrate         | 256 000 bps (fixe)            |
| Tension          | 5V (VCC)                      |
| Niveau logique   | 3.3V (compatible Pico)        |
| Fréquence radar  | 24 GHz FMCW                   |
| Portée max       | ~6 mètres                     |
| Fréquence trame  | ~10 Hz                        |

### Câblage (UART0)

```text
LD2410C TX  →  Pico GP1 (RX)
LD2410C RX  →  Pico GP0 (TX)
LD2410C VCC →  Pico VBUS (pin 40, 5V)
LD2410C GND →  Pico GND
```

---

## 📦 Installation

```toml
[dependencies]
embassy-ld2410 = "0.1.0"

[features]
rp235xa = ["embassy-ld2410/rp235xa"]  # Pico 2
rp2040  = ["embassy-ld2410/rp2040"]   # Pico 1
```

---

## 🚀 Exemple Complet

```rust
#![no_std]
#![no_main]

use embassy_executor::Spawner;
use embassy_rp::bind_interrupts;
use embassy_rp::uart::{
    BufferedUart, BufferedInterruptHandler,
    Config as UartConfig,
};
use embassy_rp::peripherals::UART0;
use embassy_ld2410::{Ld2410, TargetState};
use static_cell::StaticCell;

bind_interrupts!(struct Irqs {
    UART0_IRQ => BufferedInterruptHandler<UART0>;
});

static TX_BUF: StaticCell<[u8; 16]>  = StaticCell::new();
static RX_BUF: StaticCell<[u8; 256]> = StaticCell::new();

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    let p = embassy_rp::init(Default::default());

    let mut uart_config = UartConfig::default();
    uart_config.baudrate = 256_000;

    let uart = BufferedUart::new(
        p.UART0,
        p.PIN_0,  // TX → LD2410C RX
        p.PIN_1,  // RX → LD2410C TX
        Irqs,
        TX_BUF.init([0u8; 16]),
        RX_BUF.init([0u8; 256]),
        uart_config,
    );
    let (_tx, rx) = uart.split();
    let mut radar = Ld2410::new(rx);

    loop {
        match radar.read_presence().await {
            Ok(data) => {
                match data.target_state {
                    Some(TargetState::Moving)   => { /* mouvement détecté */ }
                    Some(TargetState::Static)   => { /* présence immobile */ }
                    Some(TargetState::Combined) => { /* mouvement + statique */ }
                    _                           => { /* aucune présence */ }
                }
                // data.detection_distance en cm
                // data.moving_energy / data.static_energy : 0-100
            }
            Err(_) => { /* réessaie automatiquement */ }
        }
    }
}
```

---

## ⚖️ Licence

Copyright (C) 2026 Jorge Andre Castro  
Distribué sous licence [GPL-2.0-or-later](https://www.gnu.org/licenses/gpl-2.0-or-later).
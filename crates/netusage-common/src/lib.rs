//! Tipos compartidos entre el lado kernel (eBPF) y el lado usuario, y las
//! comprobaciones de entorno (preflight) reutilizables por el demonio y las
//! interfaces.
//!
//! El crate es compatible con `no_std` para poder compartir el contrato de
//! contadores (`counters`) con el crate eBPF, que no dispone de `std`. El
//! módulo `preflight` usa `std` y queda detrás del feature `std` (activo por
//! defecto para el espacio de usuario).

#![cfg_attr(not(feature = "std"), no_std)]

pub mod counters;

#[cfg(feature = "std")]
pub mod preflight;

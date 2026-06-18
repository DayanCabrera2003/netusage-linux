//! Tipos compartidos entre el lado kernel (eBPF) y el lado usuario, y las
//! comprobaciones de entorno (preflight) reutilizables por el demonio y las
//! interfaces.
//!
//! El modulo `preflight` agrupa las comprobaciones de entorno.

pub mod preflight;

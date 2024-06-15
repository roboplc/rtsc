# Real-time Synchronization Components

## Introduction

Requirements for data synchronization in real-time applications differ from
traditional high-load ones. The main difference is that real-time
synchronization components must carefully respect and follow the traditional
operating-system approach, avoid user-space spin-loops and other busy-waiting
techniques (see [Channels in Rust. Part
2](https://medium.com/@disserman/channels-in-rust-part-2-603721567ee6) where
such problems are clearly described).

This crate provides a pack of real-time-safe synchronization components for
various typical and custom use-cases.

RTSC is a part of [RoboPLC](https://www.roboplc.com) project.

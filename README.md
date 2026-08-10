# Red-Black Maze

**Red-Black Maze** is a retro horror raycaster developed in **Rust** using **Raylib**.

The game is inspired by early-1990s DOS raycasters and uses a corrupted playing-card visual identity based on black, blood red, charcoal, and muted ivory.

## Current Status

The project is currently at its initial functional baseline before the architectural refactor.

The current implementation includes:

- Text-based maze loading
- Player spawn
- Player movement
- Wall collision
- Raycasting
- Field of view
- Fisheye correction
- 2D debug rendering
- 3D world rendering
- Ceiling and floor rendering
- 2D / 3D view switching

## Planned Features

The final game is planned to include:

- Three playable levels
- Playing-card wall textures: Hearts, Diamonds, Clubs, and Spades
- Animated sprites
- Enemy entities
- Horizontal mouse camera movement
- Hitscan shooting
- HUD
- Minimap
- Welcome screen
- Level selection
- Victory screen
- Conway's Game of Life menu backgrounds
- Background music
- Sound effects

## Visual Style

The project follows a:

> Retro DOS raycaster pixel art, 16-bit-style / 256-color VGA aesthetic.

The main visual identity uses:

- Black
- Charcoal
- Dark red
- Blood red
- Muted ivory
- Playing-card symbols: ♥ ♦ ♣ ♠

## Requirements

- Rust
- Cargo
- Raylib dependencies required by the `raylib` crate

## Run

```bash
cargo run
```

## Music

The planned background music is not distributed with this repository.

The local background track should be placed at:

```text
assets/audio/music/background.ogg
```

This file is intentionally excluded from version control.

## Development

The initial commit preserves the working raycaster baseline before the project is reorganized into its final architecture.

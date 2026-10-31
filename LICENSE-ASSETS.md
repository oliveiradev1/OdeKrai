# Asset Licensing and Distribution

The application code and dependencies are subject to their own license terms. Those terms do not grant permission to redistribute visual materials supplied by the owner.

## Owner-provided private materials

The 3D model and visual references supplied by the owner are private. They must not be committed or included in distributed artifacts. The application runtime and build do not consume them.

## Kenney characters

The twelve sprites used by the application are renders of Kenney's Mini Characters models, distributed under CC0. The images were rendered at 512×512 px, with transparent backgrounds and idle poses. Their source, URL, license, and modifications are recorded in [`THIRD_PARTY_NOTICES.md`](THIRD_PARTY_NOTICES.md).

The development script `scripts/render-pets.mjs` receives external source models as parameters. Three.js is a development dependency used only by this script; it is not imported by the application. Source models and textures must not be added to the repository or application artifacts.

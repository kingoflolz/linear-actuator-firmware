# Linear Actuator Firmware

A fully integrated PCB linear actuator, with the motor driver, encoder and motor all on a single PCB. All designs are
open source:

- [Blog post with commentary](https://benwang.dev/2022/08/08/PCB-Linear-Actuator.html)
- [PCB layout and schematics, mechanical components, simulation code](https://github.com/kingoflolz/linear-actuator-hardware)

# Code structure
- Embedded entrypoint: `src/main.rs`
- Host entrypoint: `host/src/main.rs`
- Libraries (FOC etc): `lib/`
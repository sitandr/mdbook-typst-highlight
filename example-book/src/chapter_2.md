# Chapter 2

It uses external Typst that should be installed in path, so you can use packages too!

```typ
#import "@preview/cetz:0.1.2": canvas, plot

#set page(width: auto, height: auto, margin: .5cm)

#canvas(length: 1cm, {
  plot.plot(size: (8, 6),
    x-tick-step: none,
    x-ticks: ((-calc.pi, $-pi$), (0, $0$), (calc.pi, $pi$)),
    y-tick-step: 1,
    {
      plot.add(
        style: plot.palette.blue,
        domain: (-calc.pi, calc.pi), x => calc.sin(x * 1rad))
      plot.add(
        hypograph: true,
        style: plot.palette.blue,
        domain: (-calc.pi, calc.pi), x => calc.cos(x * 1rad))
      plot.add(
        hypograph: true,
        style: plot.palette.blue,
        domain: (-calc.pi, calc.pi), x => calc.cos((x + calc.pi) * 1rad))
    })
})
```

```typ
$integral_a^b x^2$

#show math.equation: set text(font: "Fira Math", fallback: false)

$integral_a^b x^2$
```
# Conditional opaque candidate numerical envelope

The kernel combines independently supplied source, centroid/area, quotient, and
accumulation terms. Its absolute tail allowance and `2*area_error` term require
unit-bounded native component magnitude. Both reported streams now enforce that
requirement; invalid inputs produce no certificate and preserve baseline output.

The new ratio regression constructs independent first/second integrals and alpha
values on dyadic inputs. Perturbations vary source error, gradient, area error and
both signs. 144 scenarios yield 576 channel comparisons. The actual ratios are
computed independently of the implementation's bound expression. Separate tests
exercise all insufficient budgets, all cancellation polls and invalid domains.
These establish conditional kernel behavior; they do not establish source-route
coverage, an arbitrary opaque-tail operation bound, polygon ownership, ICC or
final display fidelity.

The five original AI pixel controls and peak managed memory match the archived
reference-interval run exactly. Two eligible routes report conditional bounds;
three retain no certificate. No baseline sampling is removed.

import SwiftUI

struct SunburstCanvas: View {
    var arcs: [ArcRecord]
    var title: String
    var onHover: (ArcRecord?) -> Void
    var onSelect: (ArcRecord) -> Void
    var onCenter: () -> Void

    var body: some View {
        GeometryReader { geo in
            Canvas { context, size in
                draw(context: context, size: size)
            }
            .contentShape(Rectangle())
            .gesture(
                DragGesture(minimumDistance: 0).onEnded { value in
                    switch classify(value.location, in: geo.size) {
                    case .slice(let arc) where !arc.path.isEmpty && !arc.isAggregate:
                        onSelect(arc)
                    case .hole:
                        onCenter()
                    case .slice, .outside:
                        break
                    }
                }
            )
            .onContinuousHover { phase in
                switch phase {
                case .active(let location):
                    if case .slice(let arc) = classify(location, in: geo.size) {
                        onHover(arc)
                    } else {
                        onHover(nil)
                    }
                case .ended:
                    onHover(nil)
                }
            }
        }
        .accessibilityElement(children: .ignore)
        .accessibilityLabel("Disk usage sunburst for \(title)")
        .accessibilityHint("Use the file list to move between folders.")
    }

    private enum Hit {
        case hole
        case slice(ArcRecord)
        case outside
    }

    private func draw(context: GraphicsContext, size: CGSize) {
        let center = CGPoint(x: size.width / 2, y: size.height / 2)
        let maxRadius = min(size.width, size.height) / 2 - 16
        guard maxRadius > 24 else { return }
        let maxDepth = arcs.map(\.depth).max() ?? 0
        let hole = maxRadius * 0.30
        let ring = maxDepth > 0 ? (maxRadius - hole) / CGFloat(maxDepth) : 0

        for arc in arcs where arc.depth > 0 {
            let inner = hole + ring * CGFloat(arc.depth - 1) + 1.5
            let outer = hole + ring * CGFloat(arc.depth) - 1.5
            guard outer > inner else { continue }
            var path = Path()
            let start = Angle.radians(arc.startAngle + 0.004)
            let end = Angle.radians(arc.endAngle - 0.004)
            path.addArc(center: center, radius: outer, startAngle: start, endAngle: end, clockwise: false)
            path.addArc(center: center, radius: inner, startAngle: end, endAngle: start, clockwise: true)
            path.closeSubpath()
            context.fill(path, with: .color(Palette.color(for: arc.category).opacity(arc.isAggregate ? 0.45 : 0.92)))
        }

        // The hole is small. A long folder name is shortened so it stays inside
        // the ring instead of drawing across the slices.
        let maxChars = max(10, Int(hole / 7))
        let shown = title.count > maxChars ? String(title.prefix(maxChars - 1)) + "…" : title
        let label = Text(shown)
            .font(.callout.weight(.semibold))
            .foregroundStyle(.primary)
        context.draw(label, at: center, anchor: .center)
    }

    private func classify(_ point: CGPoint, in size: CGSize) -> Hit {
        let center = CGPoint(x: size.width / 2, y: size.height / 2)
        let dx = point.x - center.x
        let dy = point.y - center.y
        let distance = hypot(dx, dy)
        let maxRadius = min(size.width, size.height) / 2 - 16
        let maxDepth = Int(arcs.map(\.depth).max() ?? 0)
        guard maxRadius > 24, maxDepth > 0 else { return .outside }
        let hole = maxRadius * 0.30
        if distance < hole {
            return .hole
        }
        if distance > maxRadius {
            return .outside
        }
        let ring = (maxRadius - hole) / CGFloat(maxDepth)
        let depth = min(maxDepth, max(1, Int((distance - hole) / ring) + 1))
        let angle = atan2(-dy, dx)
        if let arc = arcs.last(where: { arc in
            Int(arc.depth) == depth && angleContains(angle, start: arc.startAngle, end: arc.endAngle)
        }) {
            return .slice(arc)
        }
        return .outside
    }
}

func angleContains(_ angle: Double, start: Double, end: Double) -> Bool {
    var relative = angle - start
    let span = end - start
    let turn = Double.pi * 2
    while relative < 0 { relative += turn }
    while relative >= turn { relative -= turn }
    return relative <= span + 0.000_1
}

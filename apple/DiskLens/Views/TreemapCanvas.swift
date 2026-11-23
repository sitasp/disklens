import SwiftUI

struct TreemapCanvas: View {
    var rects: [RectRecord]
    var onHover: (RectRecord?) -> Void
    var onSelect: (RectRecord) -> Void
    var onResize: (Double, Double) -> Void

    var body: some View {
        GeometryReader { geo in
            Canvas { context, _ in
                for rect in rects {
                    let plot = CGRect(
                        x: rect.x + 1,
                        y: rect.y + 1,
                        width: max(0, rect.width - 2),
                        height: max(0, rect.height - 2)
                    )
                    let path = Path(roundedRect: plot, cornerRadius: rect.depth == 0 ? 6 : 3)
                    context.fill(
                        path,
                        with: .color(Palette.color(for: rect.category).opacity(rect.depth == 0 ? 0.88 : 0.72))
                    )
                    if rect.width > 72 && rect.height > 28 && rect.depth == 0 {
                        // Palette fills are mid-tone. Dark text stays readable on
                        // every category, including the gold document color.
                        let label = Text(rect.name)
                            .font(.caption.weight(.medium))
                            .foregroundStyle(Color.black.opacity(0.82))
                        context.draw(label, at: CGPoint(x: plot.minX + 8, y: plot.minY + 14), anchor: .leading)
                    }
                }
            }
            .contentShape(Rectangle())
            .gesture(
                DragGesture(minimumDistance: 0).onEnded { value in
                    if let rect = hit(value.location), !rect.path.isEmpty {
                        onSelect(rect)
                    }
                }
            )
            .onContinuousHover { phase in
                switch phase {
                case .active(let location):
                    onHover(hit(location))
                case .ended:
                    onHover(nil)
                }
            }
            .onChange(of: geo.size) { _, newSize in
                onResize(newSize.width, newSize.height)
            }
            .onAppear {
                onResize(geo.size.width, geo.size.height)
            }
        }
        .accessibilityElement(children: .ignore)
        .accessibilityLabel("Disk usage treemap")
        .accessibilityHint("Use the file list to move between folders.")
    }

    private func hit(_ point: CGPoint) -> RectRecord? {
        rects.last { rect in
            point.x >= rect.x && point.y >= rect.y && point.x <= rect.x + rect.width && point.y <= rect.y + rect.height
        }
    }
}

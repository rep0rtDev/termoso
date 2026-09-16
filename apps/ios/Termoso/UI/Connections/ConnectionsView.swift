import SwiftUI

/// Active terminal / SFTP / forwarding sessions. Sessions arrive with the
/// terminal milestone; this is the empty state that milestone fills in.
struct ConnectionsView: View {
    var body: some View {
        NavigationStack {
            ContentUnavailableView {
                Label("No active connections", systemImage: "bolt.horizontal")
            } description: {
                Text("Terminal, SFTP and port-forwarding sessions will appear here. iOS gives background apps only a short grace period, so sessions live while Termoso is on screen.")
            }
            .navigationTitle("Connections")
            .accessibilityIdentifier("connections.empty")
        }
    }
}

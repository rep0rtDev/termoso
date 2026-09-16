import SwiftUI
import TermosoCore

/// Hosts and sub-groups of one group (nil = vault root) with search and a
/// tag filter. Tapping a host opens the editor for now; connecting arrives
/// with the terminal milestone.
struct HostsView: View {
    @Bindable var model: VaultsModel
    let groupId: String?

    @State private var search = ""
    @State private var tagFilter: String?
    @State private var editor: HostEditorTarget?
    @State private var newGroupName: String?
    @State private var alert: String?

    private var title: String {
        groupId.flatMap { model.group($0)?.label } ?? "Hosts"
    }

    var body: some View {
        List {
            if search.isEmpty {
                let groups = model.groups(inGroup: groupId)
                if !groups.isEmpty {
                    Section("Groups") {
                        ForEach(groups, id: \.id) { group in
                            NavigationLink {
                                HostsView(model: model, groupId: group.id)
                            } label: {
                                GroupRow(group: group)
                            }
                            .accessibilityIdentifier("hosts.group.\(group.label)")
                            .swipeActions {
                                Button("Delete", role: .destructive) { model.deleteGroup(id: group.id) }
                            }
                        }
                    }
                }
            }

            let hosts = model.hosts(inGroup: groupId, matching: search, tag: tagFilter)
            Section {
                if hosts.isEmpty {
                    emptyState
                } else {
                    ForEach(hosts, id: \.id) { host in
                        Button {
                            editor = .edit(id: host.id)
                        } label: {
                            HostRow(host: host, showPath: !search.isEmpty)
                        }
                        .buttonStyle(.plain)
                        .accessibilityIdentifier("hosts.host.\(host.label)")
                        .swipeActions {
                            Button("Delete", role: .destructive) { model.deleteHost(id: host.id) }
                        }
                    }
                }
            } header: {
                if search.isEmpty, groupId == nil || !model.groups(inGroup: groupId).isEmpty {
                    Text("Hosts")
                }
            }
        }
        .listStyle(.insetGrouped)
        .navigationTitle(title)
        .searchable(text: $search, prompt: "Search hosts")
        .toolbar {
            ToolbarItemGroup(placement: .primaryAction) {
                tagMenu
                addMenu
            }
        }
        .sheet(item: $editor) { target in
            HostEditorView(model: model, target: target)
        }
        .alert("New group", isPresented: Binding(
            get: { newGroupName != nil },
            set: { if !$0 { newGroupName = nil } }
        )) {
            TextField("Name", text: Binding(get: { newGroupName ?? "" }, set: { newGroupName = $0 }))
                .accessibilityIdentifier("hosts.newGroup.name")
            Button("Cancel", role: .cancel) { newGroupName = nil }
            Button("Create") {
                let name = (newGroupName ?? "").trimmingCharacters(in: .whitespaces)
                if !name.isEmpty { model.createGroup(label: name, parentId: groupId) }
                newGroupName = nil
            }
            .accessibilityIdentifier("hosts.newGroup.create")
        }
        .alert("Error", isPresented: Binding(get: { alert != nil }, set: { if !$0 { alert = nil } })) {
            Button("OK", role: .cancel) { alert = nil }
        } message: {
            Text(alert ?? "")
        }
        .onChange(of: model.error) { _, newValue in
            if let newValue { alert = newValue }
        }
    }

    private var emptyState: some View {
        VStack(spacing: 8) {
            Image(systemName: search.isEmpty ? "server.rack" : "magnifyingglass")
                .font(.largeTitle)
                .foregroundStyle(.tertiary)
            Text(search.isEmpty ? "No hosts yet" : "Nothing matches “\(search)”")
                .font(.headline)
            if search.isEmpty {
                Text("Add a server with the + button. Everything is stored encrypted on this device.")
                    .font(.footnote)
                    .foregroundStyle(.secondary)
                    .multilineTextAlignment(.center)
            }
        }
        .frame(maxWidth: .infinity)
        .padding(.vertical, 24)
        .listRowBackground(Color.clear)
        .accessibilityIdentifier("hosts.empty")
    }

    private var addMenu: some View {
        Menu {
            Button {
                editor = .new(groupId: groupId)
            } label: {
                Label("New host", systemImage: "plus.rectangle.on.rectangle")
            }
            .accessibilityIdentifier("hosts.newHost")
            Button {
                newGroupName = ""
            } label: {
                Label("New group", systemImage: "folder.badge.plus")
            }
            .accessibilityIdentifier("hosts.newGroupButton")
        } label: {
            Image(systemName: "plus")
        }
        .accessibilityIdentifier("hosts.add")
    }

    @ViewBuilder
    private var tagMenu: some View {
        if !model.tags.isEmpty {
            Menu {
                Button {
                    tagFilter = nil
                } label: {
                    Label("All tags", systemImage: tagFilter == nil ? "checkmark" : "")
                }
                Divider()
                ForEach(model.tags, id: \.id) { tag in
                    Button {
                        tagFilter = tagFilter == tag.label ? nil : tag.label
                    } label: {
                        Label("\(tag.label) (\(tag.hosts))", systemImage: tagFilter == tag.label ? "checkmark" : "")
                    }
                }
            } label: {
                Image(systemName: tagFilter == nil ? "tag" : "tag.fill")
            }
            .accessibilityIdentifier("hosts.tagFilter")
        }
    }
}

struct GroupRow: View {
    let group: GroupItem

    var body: some View {
        HStack(spacing: 12) {
            Image(systemName: "folder.fill")
                .foregroundStyle(Color.accentColor)
                .frame(width: 28)
            VStack(alignment: .leading, spacing: 2) {
                Text(group.label)
                Text(summary)
                    .font(.caption)
                    .foregroundStyle(.secondary)
            }
        }
    }

    private var summary: String {
        var parts: [String] = []
        if group.hostCount > 0 { parts.append("\(group.hostCount) host\(group.hostCount == 1 ? "" : "s")") }
        if group.groupCount > 0 { parts.append("\(group.groupCount) group\(group.groupCount == 1 ? "" : "s")") }
        if group.hasConfig { parts.append("shared settings") }
        return parts.isEmpty ? "Empty" : parts.joined(separator: " · ")
    }
}

struct HostRow: View {
    let host: HostItem
    var showPath = false

    var body: some View {
        HStack(spacing: 12) {
            HostGlyph(host: host)
            VStack(alignment: .leading, spacing: 2) {
                Text(host.label.isEmpty ? host.address : host.label)
                    .foregroundStyle(.primary)
                Text(subtitle)
                    .font(.caption)
                    .foregroundStyle(.secondary)
                    .lineLimit(1)
                if !host.tags.isEmpty {
                    Text(host.tags.joined(separator: " · "))
                        .font(.caption2)
                        .foregroundStyle(Color.accentColor)
                        .lineLimit(1)
                }
            }
            Spacer(minLength: 0)
            if host.useMosh {
                Text("mosh")
                    .font(.caption2.weight(.semibold))
                    .padding(.horizontal, 6)
                    .padding(.vertical, 2)
                    .background(.fill.secondary, in: Capsule())
            }
        }
        .contentShape(Rectangle())
    }

    private var subtitle: String {
        var text = host.username.isEmpty ? host.address : "\(host.username)@\(host.address)"
        if host.port != 22 || host.protocol != "ssh" {
            text += ":\(host.port)"
        }
        if host.protocol != "ssh" {
            text += " · \(host.protocol)"
        }
        if showPath, !host.groupPath.isEmpty {
            text = host.groupPath.joined(separator: " / ") + " · " + text
        }
        return text
    }
}

/// Distro/OS glyph placeholder: SF Symbol by OS family. The full distro
/// icon catalog arrives with the terminal milestone.
struct HostGlyph: View {
    let host: HostItem

    var body: some View {
        Image(systemName: symbol)
            .font(.title3)
            .foregroundStyle(Color.accentColor)
            .frame(width: 32, height: 32)
            .background(.fill.tertiary, in: RoundedRectangle(cornerRadius: 8, style: .continuous))
    }

    private var symbol: String {
        let os = (host.icon ?? host.osName ?? "").lowercased()
        if os.contains("windows") { return "pc" }
        if os.contains("mac") || os.contains("darwin") { return "laptopcomputer" }
        if os.contains("bsd") { return "shield" }
        if os.contains("router") || os.contains("cisco") || os.contains("mikrotik") { return "network" }
        if os.isEmpty { return "server.rack" }
        return "terminal"
    }
}

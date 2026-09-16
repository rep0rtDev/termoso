import SwiftUI
import TermosoCore

enum HostEditorTarget: Identifiable, Hashable {
    case new(groupId: String?)
    case edit(id: String)

    var id: String {
        switch self {
        case let .new(groupId): "new:\(groupId ?? "")"
        case let .edit(id): "edit:\(id)"
        }
    }
}

/// New / edit host form over `HostDraft`. Fields the sheet does not expose
/// (proxy, jump chain, colour scheme…) are preserved by the core on save.
struct HostEditorView: View {
    @Bindable var model: VaultsModel
    let target: HostEditorTarget

    @Environment(\.dismiss) private var dismiss
    @State private var draft: HostDraft?
    @State private var password = ""
    @State private var clearPassword = false
    @State private var editPassword = false
    @State private var editTelnetPassword = false
    @State private var telnetEnabled = false
    @State private var telnet = TelnetDraft(port: nil, username: "", password: nil, identityId: nil, hasPassword: false)
    @State private var telnetPassword = ""
    @State private var newTag = ""
    @State private var error: String?
    @State private var confirmDelete = false

    var body: some View {
        NavigationStack {
            Group {
                if let draft {
                    form(draft)
                } else if let error {
                    ContentUnavailableView(error, systemImage: "exclamationmark.triangle")
                } else {
                    ProgressView()
                }
            }
            .navigationTitle(isNew ? "New host" : "Edit host")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button("Cancel") { dismiss() }
                        .accessibilityIdentifier("hostEditor.cancel")
                }
                ToolbarItem(placement: .confirmationAction) {
                    Button("Save") { save() }
                        .disabled(!canSave)
                        .accessibilityIdentifier("hostEditor.save")
                }
            }
            .alert("Couldn't save", isPresented: Binding(get: { error != nil && draft != nil }, set: { if !$0 { error = nil } })) {
                Button("OK", role: .cancel) { error = nil }
            } message: {
                Text(error ?? "")
            }
            .confirmationDialog("Delete this host?", isPresented: $confirmDelete, titleVisibility: .visible) {
                Button("Delete host", role: .destructive) {
                    if case let .edit(id) = target {
                        model.deleteHost(id: id)
                        dismiss()
                    }
                }
            }
        }
        .task { load() }
        .interactiveDismissDisabled(isDirty)
    }

    private var isNew: Bool {
        if case .new = target { return true }
        return false
    }

    private var canSave: Bool {
        guard let draft else { return false }
        let hasAddress = !draft.address.trimmingCharacters(in: .whitespaces).isEmpty
        return hasAddress && (draft.ssh || telnetEnabled)
    }

    @State private var initialSnapshot: String?
    private var isDirty: Bool {
        guard let draft, let initialSnapshot else { return false }
        return snapshot(draft) != initialSnapshot || !password.isEmpty || !telnetPassword.isEmpty
    }

    private func snapshot(_ draft: HostDraft) -> String {
        "\(draft)|\(telnetEnabled)|\(telnet)"
    }

    // MARK: Form

    @ViewBuilder
    private func form(_ current: HostDraft) -> some View {
        let binding = Binding(get: { self.draft ?? current }, set: { self.draft = $0 })
        Form {
            Section("Address") {
                TextField("Label", text: binding.label)
                    .accessibilityIdentifier("hostEditor.label")
                TextField("Hostname or IP", text: binding.address)
                    .keyboardType(.URL)
                    .textInputAutocapitalization(.never)
                    .autocorrectionDisabled()
                    .accessibilityIdentifier("hostEditor.address")
                Picker("IP version", selection: binding.ipVersion) {
                    Text("Auto").tag("auto")
                    Text("IPv4").tag("4")
                    Text("IPv6").tag("6")
                }
                groupPicker(binding)
            }

            Section {
                Toggle("SSH", isOn: binding.ssh)
                    .accessibilityIdentifier("hostEditor.ssh")
                if current.ssh {
                    OptionalNumberField("Port", value: binding.port, placeholder: "22")
                        .accessibilityIdentifier("hostEditor.port")
                    TextField("Username", text: binding.username)
                        .textInputAutocapitalization(.never)
                        .autocorrectionDisabled()
                        .textContentType(.username)
                        .accessibilityIdentifier("hostEditor.username")
                    passwordField(
                        hasStored: current.hasPassword,
                        text: $password,
                        editing: $editPassword,
                        clear: $clearPassword,
                        identifier: "hostEditor.password"
                    )
                    Toggle("Agent forwarding", isOn: binding.agentForwarding)
                    Toggle("Use Mosh", isOn: binding.useMosh)
                    if current.useMosh {
                        TextField("mosh-server command", text: Binding(
                            get: { current.moshServerCommand ?? "" },
                            set: { draft?.moshServerCommand = $0.isEmpty ? nil : $0 }
                        ))
                        .autocorrectionDisabled()
                        .textInputAutocapitalization(.never)
                    }
                }
            } header: {
                Text("SSH")
            } footer: {
                if current.ssh {
                    Text("Keys, identities and SSH ID arrive with the Keychain milestone; password auth works now.")
                }
            }

            Section {
                Toggle("Telnet", isOn: $telnetEnabled)
                    .accessibilityIdentifier("hostEditor.telnet")
                if telnetEnabled {
                    OptionalNumberField("Port", value: $telnet.port, placeholder: "23")
                    TextField("Username", text: $telnet.username)
                        .textInputAutocapitalization(.never)
                        .autocorrectionDisabled()
                    passwordField(
                        hasStored: telnet.hasPassword,
                        text: $telnetPassword,
                        editing: $editTelnetPassword,
                        clear: Binding(
                            get: { telnet.password == "" },
                            set: { telnet.password = $0 ? "" : nil }
                        ),
                        identifier: "hostEditor.telnetPassword"
                    )
                }
            } header: {
                Text("Telnet")
            }

            tagsSection(binding)

            Section("Advanced") {
                OptionalNumberField("Keep-alive (s)", value: binding.keepAliveInterval, placeholder: "off")
                OptionalNumberField("Timeout (s)", value: binding.timeout, placeholder: "default")
                TextField("Notes", text: binding.notes, axis: .vertical)
                    .lineLimit(2 ... 6)
            }

            if !isNew {
                Section {
                    Button("Delete host", role: .destructive) { confirmDelete = true }
                        .frame(maxWidth: .infinity)
                        .accessibilityIdentifier("hostEditor.delete")
                }
            }
        }
    }

    private func groupPicker(_ binding: Binding<HostDraft>) -> some View {
        Picker("Group", selection: binding.groupId) {
            Text("None").tag(String?.none)
            ForEach(groupPaths(), id: \.id) { entry in
                Text(entry.path).tag(Optional(entry.id))
            }
        }
        .accessibilityIdentifier("hostEditor.group")
    }

    private func groupPaths() -> [(id: String, path: String)] {
        func path(of group: GroupItem) -> String {
            var parts = [group.label]
            var parent = group.parentId
            var guardCount = 0
            while let id = parent, let g = model.group(id), guardCount < 32 {
                parts.insert(g.label, at: 0)
                parent = g.parentId
                guardCount += 1
            }
            return parts.joined(separator: " / ")
        }
        return model.groups.map { (id: $0.id, path: path(of: $0)) }
            .sorted { $0.path.localizedCaseInsensitiveCompare($1.path) == .orderedAscending }
    }

    @ViewBuilder
    private func passwordField(
        hasStored: Bool,
        text: Binding<String>,
        editing: Binding<Bool>,
        clear: Binding<Bool>,
        identifier: String
    ) -> some View {
        if hasStored, !clear.wrappedValue, !editing.wrappedValue {
            HStack {
                Label("Password saved", systemImage: "lock.fill")
                    .foregroundStyle(.secondary)
                Spacer()
                Button("Change") { editing.wrappedValue = true }
                    .font(.footnote)
                Button("Remove", role: .destructive) { clear.wrappedValue = true }
                    .font(.footnote)
            }
            .buttonStyle(.borderless)
        } else {
            SecureField(clear.wrappedValue ? "Password removed on save" : "Password", text: text)
                .textContentType(.password)
                .accessibilityIdentifier(identifier)
        }
    }

    private func tagsSection(_ binding: Binding<HostDraft>) -> some View {
        Section("Tags") {
            ForEach(model.tags, id: \.id) { tag in
                let selected = binding.wrappedValue.tagIds.contains(tag.id)
                Button {
                    if selected {
                        draft?.tagIds.removeAll { $0 == tag.id }
                    } else {
                        draft?.tagIds.append(tag.id)
                    }
                } label: {
                    HStack {
                        Text(tag.label).foregroundStyle(.primary)
                        Spacer()
                        if selected {
                            Image(systemName: "checkmark").foregroundStyle(Color.accentColor)
                        }
                    }
                }
                .accessibilityIdentifier("hostEditor.tag.\(tag.label)")
            }
            HStack {
                TextField("New tag", text: $newTag)
                    .textInputAutocapitalization(.never)
                    .onSubmit(addTag)
                    .accessibilityIdentifier("hostEditor.newTag")
                Button("Add", action: addTag)
                    .disabled(newTag.trimmingCharacters(in: .whitespaces).isEmpty)
                    .accessibilityIdentifier("hostEditor.addTag")
            }
        }
    }

    // MARK: Actions

    private func load() {
        do {
            let loaded: HostDraft
            switch target {
            case let .new(groupId):
                loaded = try model.newHostDraft(groupId: groupId)
            case let .edit(id):
                loaded = try model.hostDraft(id: id)
            }
            if let t = loaded.telnet {
                telnetEnabled = true
                telnet = t
            }
            draft = loaded
            initialSnapshot = snapshot(loaded)
        } catch {
            self.error = userMessage(for: error)
        }
    }

    private func addTag() {
        let label = newTag.trimmingCharacters(in: .whitespaces)
        guard !label.isEmpty else { return }
        do {
            let tag = try model.createTag(label: label)
            if draft?.tagIds.contains(tag.id) == false {
                draft?.tagIds.append(tag.id)
            }
            newTag = ""
        } catch {
            self.error = userMessage(for: error)
        }
    }

    private func save() {
        guard var out = draft else { return }
        out.label = out.label.trimmingCharacters(in: .whitespaces)
        out.address = out.address.trimmingCharacters(in: .whitespaces)
        out.username = out.username.trimmingCharacters(in: .whitespaces)
        if clearPassword {
            out.password = ""
        } else if !password.isEmpty {
            out.password = password
        } else {
            out.password = nil
        }
        if telnetEnabled {
            var t = telnet
            t.username = t.username.trimmingCharacters(in: .whitespaces)
            if !telnetPassword.isEmpty {
                t.password = telnetPassword
            }
            out.telnet = t
        } else {
            out.telnet = nil
        }
        do {
            try model.saveHost(out)
            dismiss()
        } catch {
            self.error = userMessage(for: error)
        }
    }
}

/// Text field bound to an optional integer; empty = nil.
struct OptionalNumberField<Value: FixedWidthInteger>: View {
    let title: String
    @Binding var value: Value?
    let placeholder: String

    init(_ title: String, value: Binding<Value?>, placeholder: String) {
        self.title = title
        _value = value
        self.placeholder = placeholder
    }

    var body: some View {
        HStack {
            Text(title)
            Spacer()
            TextField(placeholder, text: Binding(
                get: { value.map { String($0) } ?? "" },
                set: { text in
                    let digits = text.filter(\.isNumber)
                    value = digits.isEmpty ? nil : Value(digits)
                }
            ))
            .keyboardType(.numberPad)
            .multilineTextAlignment(.trailing)
            .frame(maxWidth: 120)
        }
    }
}

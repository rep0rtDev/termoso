package com.termoso.android.ui.shell

import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.padding
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Cable
import androidx.compose.material.icons.filled.Lock
import androidx.compose.material.icons.filled.Settings
import androidx.compose.material.icons.outlined.Cable
import androidx.compose.material.icons.outlined.Lock
import androidx.compose.material.icons.outlined.Settings
import androidx.compose.material3.Icon
import androidx.compose.material3.NavigationBar
import androidx.compose.material3.NavigationBarItem
import androidx.compose.material3.Scaffold
import androidx.compose.material3.SnackbarHost
import androidx.compose.material3.SnackbarHostState
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.lifecycle.viewmodel.compose.viewModel
import androidx.navigation.NavGraph.Companion.findStartDestination
import androidx.navigation.NavType
import androidx.navigation.compose.NavHost
import androidx.navigation.compose.composable
import androidx.navigation.compose.currentBackStackEntryAsState
import androidx.navigation.compose.rememberNavController
import androidx.navigation.navArgument
import com.termoso.android.data.AccountManager
import com.termoso.android.data.AppContainer
import com.termoso.android.data.ForwardManager
import com.termoso.android.data.PresenceManager
import com.termoso.android.data.SessionManager
import com.termoso.android.data.SftpConnection
import com.termoso.android.data.SftpManager
import com.termoso.android.data.VaultRepository
import com.termoso.android.ui.account.AccountScreen
import com.termoso.android.ui.account.AuthMode
import com.termoso.android.ui.account.ReauthHost
import com.termoso.android.ui.account.SecurityKeysScreen
import com.termoso.android.ui.account.SignInScreen
import com.termoso.android.ui.account.SshIdFido2Screen
import com.termoso.android.ui.account.SshIdScreen
import com.termoso.android.ui.connections.ConnectionsScreen
import com.termoso.android.ui.forwarding.ForwardEditorScreen
import com.termoso.android.ui.forwarding.ForwardWizardScreen
import com.termoso.android.ui.forwarding.ForwardingScreen
import com.termoso.android.ui.forwarding.TunnelPromptHost
import com.termoso.android.ui.hosts.HostEditorScreen
import com.termoso.android.ui.hosts.HostsScreen
import com.termoso.android.ui.keychain.Fido2GenerateScreen
import com.termoso.android.ui.keychain.Fido2LoadScreen
import com.termoso.android.ui.keychain.GenerateKeyScreen
import com.termoso.android.ui.keychain.IdentityEditorScreen
import com.termoso.android.ui.keychain.ImportKeyScreen
import com.termoso.android.ui.keychain.KeyDetailScreen
import com.termoso.android.ui.keychain.KeychainScreen
import com.termoso.android.ui.settings.SettingsScreen
import com.termoso.android.ui.sftp.SftpPickScreen
import com.termoso.android.ui.sftp.SftpScreen
import com.termoso.android.ui.snippets.SnippetEditorScreen
import com.termoso.android.ui.snippets.SnippetsScreen
import com.termoso.android.ui.settings.TerminalAppearanceScreen
import com.termoso.android.ui.team.TeamActivityScreen
import com.termoso.android.ui.team.TeamScreen
import com.termoso.android.ui.team.TeamVaultScreen
import com.termoso.android.ui.team.TeamsScreen
import com.termoso.android.ui.terminal.TerminalScreen
import com.termoso.android.ui.vault.HistoryScreen
import com.termoso.android.ui.vault.KnownHostsScreen
import com.termoso.android.ui.vault.SessionLogScreen
import com.termoso.android.ui.vault.SessionLogsScreen
import com.termoso.android.ui.vault.VaultScreen
import com.termoso.core.KeyMods
import com.termoso.core.PfKind
import com.termoso.core.Transport
import kotlinx.coroutines.launch

object Routes {
    const val VAULT = "vault"
    const val CONNECTIONS = "connections"
    const val SETTINGS = "settings"
    const val HOSTS = "hosts?group={group}"
    const val HOST_NEW = "hostNew?group={group}"
    const val HOST_EDIT = "hostEdit/{id}"
    const val KEYCHAIN = "keychain"
    const val KEY_GENERATE = "keyGenerate"
    const val KEY_IMPORT = "keyImport"
    const val KEY_FIDO2 = "keyFido2"
    const val KEY_FIDO2_LOAD = "keyFido2Load"
    const val KEY_DETAIL = "key/{id}"
    const val IDENTITY_NEW = "identityNew"
    const val IDENTITY_EDIT = "identity/{id}"
    const val TERMINAL_APPEARANCE = "terminalAppearance"
    const val KNOWN_HOSTS = "knownHosts"
    const val HISTORY = "history"
    const val LOGS = "logs"
    const val LOG = "log/{id}"
    const val TERMINAL = "terminal"
    const val ACCOUNT = "account"
    const val TEAMS = "teams"
    const val SSH_ID = "sshId"
    const val SSH_ID_FIDO2 = "sshIdFido2"
    const val SECURITY_KEYS = "securityKeys"
    const val TEAM = "team/{id}"
    const val TEAM_ACTIVITY = "team/{id}/activity"
    const val TEAM_VAULT = "teamVault/{id}"
    const val SIGN_IN = "signIn/{mode}"
    const val SFTP = "sftp/{id}"
    const val SFTP_PICK = "sftpPick"
    const val FORWARDING = "forwarding"
    const val PF_WIZARD = "pfWizard"
    const val PF_NEW = "pfNew?kind={kind}&vault={vault}&host={host}"
    const val PF_EDIT = "pfEdit/{id}"
    const val SNIPPETS = "snippets?pkg={pkg}"
    const val SNIPPET_NEW = "snippetNew?vault={vault}&pkg={pkg}"
    const val SNIPPET_EDIT = "snippetEdit/{id}"

    fun hosts(group: String?) = if (group == null) "hosts" else "hosts?group=$group"
    fun hostNew(group: String?) = if (group == null) "hostNew" else "hostNew?group=$group"
    fun hostEdit(id: String) = "hostEdit/$id"
    fun key(id: String) = "key/$id"
    fun identity(id: String) = "identity/$id"
    fun log(id: String) = "log/$id"
    fun signIn(mode: AuthMode) = "signIn/${mode.name}"
    fun team(id: String) = "team/$id"
    fun teamActivity(id: String) = "team/$id/activity"
    fun teamVault(id: String) = "teamVault/$id"
    fun sftp(id: String) = "sftp/$id"
    fun pfNew(kind: PfKind, vault: String?, host: String?) =
        "pfNew?kind=${kind.name}" + (vault?.let { "&vault=$it" } ?: "") + (host?.let { "&host=$it" } ?: "")
    fun pfEdit(id: String) = "pfEdit/$id"
    fun snippets(pkg: String?) = if (pkg == null) "snippets" else "snippets?pkg=$pkg"
    fun snippetNew(vault: String, pkg: String?) = "snippetNew?vault=$vault" + (pkg?.let { "&pkg=$it" } ?: "")
    fun snippetEdit(id: String) = "snippetEdit/$id"
}

private class Tab(val route: String, val label: String, val icon: ImageVector, val selectedIcon: ImageVector)

private val tabs = listOf(
    Tab(Routes.VAULT, "Vaults", Icons.Outlined.Lock, Icons.Filled.Lock),
    Tab(Routes.CONNECTIONS, "Connections", Icons.Outlined.Cable, Icons.Filled.Cable),
    Tab(Routes.SETTINGS, "Settings", Icons.Outlined.Settings, Icons.Filled.Settings),
)

/** Bottom-navigation shell: Vaults · Connections · Settings, with nested host screens. */
@Composable
fun MainShell(
    container: AppContainer,
    repo: VaultRepository,
    sessions: SessionManager,
    sftp: SftpManager,
    forwards: ForwardManager,
    account: AccountManager,
    presence: PresenceManager,
    onLock: () -> Unit,
) {
    val shell: ShellViewModel = viewModel { ShellViewModel(repo, sessions, sftp, forwards, presence) }
    val nav = rememberNavController()
    val scope = rememberCoroutineScope()
    val backStack by nav.currentBackStackEntryAsState()
    val currentRoute = backStack?.destination?.route
    val showBar = tabs.any { it.route == currentRoute }
    val snackbar = remember { SnackbarHostState() }

    val notice by shell.notice.collectAsStateWithLifecycle()
    LaunchedEffect(notice) {
        val text = notice ?: return@LaunchedEffect
        snackbar.showSnackbar(text)
        shell.noticeShown()
    }
    LaunchedEffect(account) {
        account.notices.collect { snackbar.showSnackbar(it) }
    }
    LaunchedEffect(forwards) { forwards.autoStartOnce() }
    TunnelPromptHost(forwards)
    ReauthHost(account)

    val pendingInvite by container.pendingInvite.collectAsStateWithLifecycle()
    val accountStatus by account.status.collectAsStateWithLifecycle()
    LaunchedEffect(pendingInvite, accountStatus.account?.userId) {
        if (pendingInvite == null) return@LaunchedEffect
        if (accountStatus.account == null) {
            shell.notify("Sign in to accept the team invitation")
            nav.navigate(Routes.signIn(AuthMode.SignIn)) { launchSingleTop = true }
        } else if (nav.currentDestination?.route != Routes.TEAMS) {
            // Re-navigating onto the visible Teams screen would recreate it and
            // drop the dialog its previous instance just opened for this link.
            nav.navigate(Routes.TEAMS) { launchSingleTop = true }
        }
    }

    fun openTerminal() {
        nav.navigate(Routes.TERMINAL) { launchSingleTop = true }
    }

    // A termoso://join link: needs an account, then becomes a viewer terminal.
    val pendingJoin by container.pendingJoin.collectAsStateWithLifecycle()
    val restoring by account.restoring.collectAsStateWithLifecycle()
    LaunchedEffect(pendingJoin, accountStatus.account?.userId, restoring) {
        val link = pendingJoin ?: return@LaunchedEffect
        if (restoring) return@LaunchedEffect
        if (accountStatus.account == null) {
            shell.notify("Sign in to join the shared terminal")
            nav.navigate(Routes.signIn(AuthMode.SignIn)) { launchSingleTop = true }
            return@LaunchedEffect
        }
        // Consuming re-keys this effect, so the join itself runs on the shell scope.
        container.consumeJoin()
        scope.launch { if (shell.joinLive(link) != null) openTerminal() }
    }

    // Files shared into the app: sent to the active terminal, or held until one is opened.
    val pendingShare by container.pendingShare.collectAsStateWithLifecycle()
    val openSessions by shell.sessions.sessions.collectAsStateWithLifecycle()
    LaunchedEffect(pendingShare, openSessions.isEmpty()) {
        if (pendingShare == null) return@LaunchedEffect
        if (openSessions.isEmpty()) {
            shell.notify("Connect to a host; the shared files will be sent to that terminal")
            nav.navigate(Routes.CONNECTIONS) {
                popUpTo(nav.graph.findStartDestination().id) { saveState = true }
                launchSingleTop = true
                restoreState = true
            }
        } else if (nav.currentDestination?.route != Routes.TERMINAL) {
            openTerminal()
        }
    }

    fun connectHost(hostId: String, transport: Transport = Transport.AUTO) {
        scope.launch { if (shell.connectHost(hostId, transport) != null) openTerminal() }
    }

    fun openSftp(connectionId: String) {
        nav.navigate(Routes.sftp(connectionId)) { launchSingleTop = true }
    }

    fun sftpHost(hostId: String) {
        scope.launch { shell.openSftpHost(hostId)?.let { openSftp(it.id) } }
    }

    /** Termius-style "Edit": a terminal to the same host running an editor on the file, exiting with it. */
    fun editInTerminal(conn: SftpConnection, path: String) {
        scope.launch {
            val session = when {
                conn.hostId != null -> shell.connectHost(conn.hostId)
                conn.quick != null -> shell.connectQuick(conn.quick)
                else -> null
            } ?: return@launch
            val quoted = "'" + path.replace("'", "'\\''") + "'"
            session.rust.sendText("\${EDITOR:-vi} $quoted; exit\n", KeyMods(ctrl = false, alt = false, shift = false))
            openTerminal()
        }
    }

    Scaffold(
        contentWindowInsets = WindowInsets(0),
        snackbarHost = { SnackbarHost(snackbar) },
        bottomBar = {
            if (showBar) {
                NavigationBar {
                    tabs.forEach { tab ->
                        val selected = currentRoute == tab.route
                        NavigationBarItem(
                            selected = selected,
                            onClick = {
                                nav.navigate(tab.route) {
                                    popUpTo(nav.graph.findStartDestination().id) { saveState = true }
                                    launchSingleTop = true
                                    restoreState = true
                                }
                            },
                            icon = { Icon(if (selected) tab.selectedIcon else tab.icon, contentDescription = tab.label) },
                            label = { Text(tab.label) },
                        )
                    }
                }
            }
        },
    ) { padding ->
        val groupArg = navArgument("group") { type = NavType.StringType; nullable = true; defaultValue = null }
        val idArg = navArgument("id") { type = NavType.StringType }
        NavHost(
            navController = nav,
            startDestination = Routes.VAULT,
            modifier = Modifier.padding(bottom = if (showBar) padding.calculateBottomPadding() else 0.dp),
        ) {
            composable(Routes.VAULT) {
                VaultScreen(
                    shell = shell,
                    account = account,
                    onOpenAccount = { nav.navigate(Routes.ACCOUNT) },
                    onSignIn = { nav.navigate(Routes.signIn(AuthMode.SignIn)) },
                    onOpenHosts = { nav.navigate(Routes.hosts(null)) },
                    onOpenKeychain = { nav.navigate(Routes.KEYCHAIN) },
                    onOpenForwarding = { nav.navigate(Routes.FORWARDING) },
                    onOpenSnippets = { nav.navigate(Routes.snippets(null)) },
                    onOpenKnownHosts = { nav.navigate(Routes.KNOWN_HOSTS) },
                    onOpenHistory = { nav.navigate(Routes.HISTORY) },
                    onOpenLogs = { nav.navigate(Routes.LOGS) },
                )
            }
            composable(Routes.CONNECTIONS) {
                ConnectionsScreen(
                    shell = shell,
                    onAddHost = { nav.navigate(Routes.hostNew(null)) },
                    onConnectHost = { connectHost(it) },
                    onOpenTerminal = ::openTerminal,
                    onNewSftp = { nav.navigate(Routes.SFTP_PICK) },
                    onOpenSftp = ::openSftp,
                )
            }
            composable(Routes.SETTINGS) {
                SettingsScreen(
                    shell = shell,
                    container = container,
                    account = account,
                    onAccount = { nav.navigate(Routes.ACCOUNT) },
                    onSignIn = { nav.navigate(Routes.signIn(AuthMode.SignIn)) },
                    onLock = onLock,
                    onTerminalAppearance = { nav.navigate(Routes.TERMINAL_APPEARANCE) },
                )
            }
            composable(Routes.TERMINAL_APPEARANCE) { TerminalAppearanceScreen(shell = shell, onBack = { nav.popBackStack() }) }
            composable(Routes.ACCOUNT) {
                AccountScreen(
                    shell = shell,
                    account = account,
                    onBack = { nav.popBackStack() },
                    onSignedOut = { nav.popBackStack() },
                    onTeams = { nav.navigate(Routes.TEAMS) },
                    onSshId = { nav.navigate(Routes.SSH_ID) },
                    onSecurityKeys = { nav.navigate(Routes.SECURITY_KEYS) },
                )
            }
            composable(Routes.SSH_ID) {
                SshIdScreen(
                    shell = shell,
                    account = account,
                    onBack = { nav.popBackStack() },
                    onAddSecurityKey = { nav.navigate(Routes.SSH_ID_FIDO2) },
                )
            }
            composable(Routes.SSH_ID_FIDO2) {
                SshIdFido2Screen(shell = shell, account = account, onClose = { nav.popBackStack() }, onDone = { nav.popBackStack() })
            }
            composable(Routes.SECURITY_KEYS) {
                SecurityKeysScreen(shell = shell, account = account, onBack = { nav.popBackStack() })
            }
            composable(Routes.TEAMS) {
                TeamsScreen(
                    shell = shell,
                    onBack = { nav.popBackStack() },
                    onOpenTeam = { nav.navigate(Routes.team(it)) },
                    joinLink = pendingInvite,
                    onJoinLinkShown = container::consumeInvite,
                )
            }
            composable(Routes.TEAM, arguments = listOf(idArg)) { entry ->
                val id = entry.arguments?.getString("id") ?: return@composable
                TeamScreen(
                    shell = shell,
                    account = account,
                    teamId = id,
                    onBack = { nav.popBackStack() },
                    onOpenVault = { nav.navigate(Routes.teamVault(it)) },
                    onActivity = { nav.navigate(Routes.teamActivity(id)) },
                )
            }
            composable(Routes.TEAM_ACTIVITY, arguments = listOf(idArg)) { entry ->
                val id = entry.arguments?.getString("id") ?: return@composable
                TeamActivityScreen(shell = shell, teamId = id, onBack = { nav.popBackStack() })
            }
            composable(Routes.TEAM_VAULT, arguments = listOf(idArg)) { entry ->
                val id = entry.arguments?.getString("id") ?: return@composable
                TeamVaultScreen(shell = shell, account = account, vaultId = id, onBack = { nav.popBackStack() })
            }
            composable(Routes.SIGN_IN, arguments = listOf(navArgument("mode") { type = NavType.StringType })) { entry ->
                val mode = entry.arguments?.getString("mode")?.let { m -> AuthMode.entries.firstOrNull { it.name == m } } ?: AuthMode.SignIn
                SignInScreen(
                    account = account,
                    mode = mode,
                    onBack = { nav.popBackStack() },
                    onDone = { nav.navigate(Routes.ACCOUNT) { popUpTo(Routes.SETTINGS) } },
                )
            }
            composable(Routes.HOSTS, arguments = listOf(groupArg)) { entry ->
                val group = entry.arguments?.getString("group")
                HostsScreen(
                    shell = shell,
                    groupId = group,
                    onBack = { nav.popBackStack() },
                    onOpenGroup = { nav.navigate(Routes.hosts(it)) },
                    onNewHost = { nav.navigate(Routes.hostNew(group)) },
                    onEditHost = { nav.navigate(Routes.hostEdit(it)) },
                    onConnect = { connectHost(it) },
                    onConnectWith = ::connectHost,
                    onSftp = ::sftpHost,
                    onForward = { nav.navigate(Routes.pfNew(PfKind.LOCAL, null, it)) },
                )
            }
            composable(Routes.HOST_NEW, arguments = listOf(groupArg)) { entry ->
                HostEditorScreen(
                    shell = shell,
                    hostId = null,
                    groupId = entry.arguments?.getString("group"),
                    onClose = { nav.popBackStack() },
                )
            }
            composable(Routes.HOST_EDIT, arguments = listOf(navArgument("id") { type = NavType.StringType })) { entry ->
                HostEditorScreen(
                    shell = shell,
                    hostId = entry.arguments?.getString("id"),
                    groupId = null,
                    onClose = { nav.popBackStack() },
                )
            }
            composable(Routes.KEYCHAIN) {
                KeychainScreen(
                    shell = shell,
                    onBack = { nav.popBackStack() },
                    onGenerate = { nav.navigate(Routes.KEY_GENERATE) },
                    onImport = { nav.navigate(Routes.KEY_IMPORT) },
                    onFido2 = { nav.navigate(Routes.KEY_FIDO2) },
                    onFido2Load = { nav.navigate(Routes.KEY_FIDO2_LOAD) },
                    onOpenKey = { nav.navigate(Routes.key(it)) },
                    onNewIdentity = { nav.navigate(Routes.IDENTITY_NEW) },
                    onOpenIdentity = { nav.navigate(Routes.identity(it)) },
                )
            }
            composable(Routes.KEY_GENERATE) {
                GenerateKeyScreen(shell = shell, onClose = { nav.popBackStack() }, onSaved = { id ->
                    nav.navigate(Routes.key(id)) { popUpTo(Routes.KEYCHAIN) }
                })
            }
            composable(Routes.KEY_IMPORT) {
                ImportKeyScreen(shell = shell, onClose = { nav.popBackStack() }, onSaved = { id ->
                    nav.navigate(Routes.key(id)) { popUpTo(Routes.KEYCHAIN) }
                })
            }
            composable(Routes.KEY_FIDO2) {
                Fido2GenerateScreen(shell = shell, onClose = { nav.popBackStack() }, onSaved = { id ->
                    nav.navigate(Routes.key(id)) { popUpTo(Routes.KEYCHAIN) }
                })
            }
            composable(Routes.KEY_FIDO2_LOAD) {
                Fido2LoadScreen(shell = shell, onClose = { nav.popBackStack() }, onLoaded = { ids ->
                    if (ids.size == 1) nav.navigate(Routes.key(ids.single())) { popUpTo(Routes.KEYCHAIN) } else nav.popBackStack()
                })
            }
            composable(Routes.KEY_DETAIL, arguments = listOf(idArg)) { entry ->
                KeyDetailScreen(shell = shell, keyId = entry.arguments?.getString("id") ?: "", onBack = { nav.popBackStack() })
            }
            composable(Routes.IDENTITY_NEW) { IdentityEditorScreen(shell = shell, identityId = null, onClose = { nav.popBackStack() }) }
            composable(Routes.IDENTITY_EDIT, arguments = listOf(idArg)) { entry ->
                IdentityEditorScreen(shell = shell, identityId = entry.arguments?.getString("id"), onClose = { nav.popBackStack() })
            }
            composable(Routes.KNOWN_HOSTS) { KnownHostsScreen(shell = shell, onBack = { nav.popBackStack() }) }
            composable(Routes.HISTORY) {
                HistoryScreen(shell = shell, onBack = { nav.popBackStack() }, onOpenHost = { nav.navigate(Routes.hostEdit(it)) })
            }
            composable(Routes.LOGS) {
                SessionLogsScreen(shell = shell, onBack = { nav.popBackStack() }, onOpen = { nav.navigate(Routes.log(it)) })
            }
            composable(Routes.LOG, arguments = listOf(idArg)) { entry ->
                SessionLogScreen(shell = shell, logId = entry.arguments?.getString("id") ?: "", onBack = { nav.popBackStack() })
            }
            composable(Routes.FORWARDING) {
                ForwardingScreen(
                    shell = shell,
                    onBack = { nav.popBackStack() },
                    onNewRule = { nav.navigate(Routes.PF_WIZARD) },
                    onEditRule = { nav.navigate(Routes.pfEdit(it)) },
                )
            }
            composable(Routes.PF_WIZARD) {
                ForwardWizardScreen(
                    shell = shell,
                    onBack = { nav.popBackStack() },
                    onContinue = { kind, vault ->
                        nav.navigate(Routes.pfNew(kind, vault, null)) { popUpTo(Routes.PF_WIZARD) { inclusive = true } }
                    },
                )
            }
            val optArg = { name: String -> navArgument(name) { type = NavType.StringType; nullable = true; defaultValue = null } }
            composable(Routes.PF_NEW, arguments = listOf(optArg("kind"), optArg("vault"), optArg("host"))) { entry ->
                val kind = entry.arguments?.getString("kind")?.let { k -> PfKind.entries.firstOrNull { it.name == k } } ?: PfKind.LOCAL
                ForwardEditorScreen(
                    shell = shell,
                    ruleId = null,
                    kind = kind,
                    vaultId = entry.arguments?.getString("vault"),
                    hostId = entry.arguments?.getString("host"),
                    onClose = { nav.popBackStack() },
                )
            }
            composable(Routes.PF_EDIT, arguments = listOf(idArg)) { entry ->
                ForwardEditorScreen(
                    shell = shell,
                    ruleId = entry.arguments?.getString("id") ?: "",
                    kind = PfKind.LOCAL,
                    vaultId = null,
                    hostId = null,
                    onClose = { nav.popBackStack() },
                )
            }
            composable(Routes.SNIPPETS, arguments = listOf(optArg("pkg"))) { entry ->
                SnippetsScreen(
                    shell = shell,
                    packageId = entry.arguments?.getString("pkg"),
                    onBack = { nav.popBackStack() },
                    onOpenPackage = { nav.navigate(Routes.snippets(it)) },
                    onNewSnippet = { vault, pkg -> nav.navigate(Routes.snippetNew(vault, pkg)) },
                    onEditSnippet = { nav.navigate(Routes.snippetEdit(it)) },
                    onOpenTerminal = ::openTerminal,
                )
            }
            composable(Routes.SNIPPET_NEW, arguments = listOf(optArg("vault"), optArg("pkg"))) { entry ->
                SnippetEditorScreen(
                    shell = shell,
                    snippetId = null,
                    vaultId = entry.arguments?.getString("vault"),
                    packageId = entry.arguments?.getString("pkg"),
                    onClose = { nav.popBackStack() },
                )
            }
            composable(Routes.SNIPPET_EDIT, arguments = listOf(idArg)) { entry ->
                SnippetEditorScreen(
                    shell = shell,
                    snippetId = entry.arguments?.getString("id") ?: "",
                    vaultId = null,
                    packageId = null,
                    onClose = { nav.popBackStack() },
                )
            }
            composable(Routes.SFTP_PICK) {
                SftpPickScreen(
                    shell = shell,
                    onBack = { nav.popBackStack() },
                    onOpened = { id -> nav.navigate(Routes.sftp(id)) { popUpTo(Routes.SFTP_PICK) { inclusive = true } } },
                )
            }
            composable(Routes.SFTP, arguments = listOf(idArg)) { entry ->
                SftpScreen(
                    shell = shell,
                    connectionId = entry.arguments?.getString("id") ?: "",
                    onBack = { nav.popBackStack() },
                    onEdit = ::editInTerminal,
                )
            }
            composable(Routes.TERMINAL) {
                TerminalScreen(
                    shell = shell,
                    onBack = { nav.popBackStack() },
                    onOpenSnippets = { nav.navigate(Routes.snippets(null)) },
                    pendingShare = pendingShare,
                    onShareConsumed = { container.consumeShare() },
                    onNewSession = {
                        // Leave the terminal first so it is not part of the
                        // saved tab state that restoreState would bring back.
                        nav.popBackStack()
                        nav.navigate(Routes.CONNECTIONS) {
                            popUpTo(nav.graph.findStartDestination().id) { saveState = true }
                            launchSingleTop = true
                            restoreState = true
                        }
                    },
                )
            }
        }
    }
}

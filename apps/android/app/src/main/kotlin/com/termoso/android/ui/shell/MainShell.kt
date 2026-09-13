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
import com.termoso.android.data.VaultRepository
import com.termoso.android.ui.connections.ConnectionsScreen
import com.termoso.android.ui.hosts.HostEditorScreen
import com.termoso.android.ui.hosts.HostsScreen
import com.termoso.android.ui.settings.SettingsScreen
import com.termoso.android.ui.vault.HistoryScreen
import com.termoso.android.ui.vault.KeychainScreen
import com.termoso.android.ui.vault.KnownHostsScreen
import com.termoso.android.ui.vault.VaultScreen

object Routes {
    const val VAULT = "vault"
    const val CONNECTIONS = "connections"
    const val SETTINGS = "settings"
    const val HOSTS = "hosts?group={group}"
    const val HOST_NEW = "hostNew?group={group}"
    const val HOST_EDIT = "hostEdit/{id}"
    const val KEYCHAIN = "keychain"
    const val KNOWN_HOSTS = "knownHosts"
    const val HISTORY = "history"

    fun hosts(group: String?) = if (group == null) "hosts" else "hosts?group=$group"
    fun hostNew(group: String?) = if (group == null) "hostNew" else "hostNew?group=$group"
    fun hostEdit(id: String) = "hostEdit/$id"
}

private class Tab(val route: String, val label: String, val icon: ImageVector, val selectedIcon: ImageVector)

private val tabs = listOf(
    Tab(Routes.VAULT, "Vaults", Icons.Outlined.Lock, Icons.Filled.Lock),
    Tab(Routes.CONNECTIONS, "Connections", Icons.Outlined.Cable, Icons.Filled.Cable),
    Tab(Routes.SETTINGS, "Settings", Icons.Outlined.Settings, Icons.Filled.Settings),
)

/** Bottom-navigation shell: Vaults · Connections · Settings, with nested host screens. */
@Composable
fun MainShell(repo: VaultRepository, onCloud: () -> Unit, onLock: () -> Unit) {
    val shell: ShellViewModel = viewModel { ShellViewModel(repo) }
    val nav = rememberNavController()
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
        NavHost(
            navController = nav,
            startDestination = Routes.VAULT,
            modifier = Modifier.padding(bottom = if (showBar) padding.calculateBottomPadding() else 0.dp),
        ) {
            composable(Routes.VAULT) {
                VaultScreen(
                    shell = shell,
                    onOpenHosts = { nav.navigate(Routes.hosts(null)) },
                    onOpenKeychain = { nav.navigate(Routes.KEYCHAIN) },
                    onOpenKnownHosts = { nav.navigate(Routes.KNOWN_HOSTS) },
                    onOpenHistory = { nav.navigate(Routes.HISTORY) },
                )
            }
            composable(Routes.CONNECTIONS) {
                ConnectionsScreen(
                    shell = shell,
                    onAddHost = { nav.navigate(Routes.hostNew(null)) },
                    onOpenHost = { nav.navigate(Routes.hostEdit(it)) },
                )
            }
            composable(Routes.SETTINGS) {
                SettingsScreen(shell = shell, onCloud = onCloud, onLock = onLock)
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
            composable(Routes.KEYCHAIN) { KeychainScreen(shell = shell, onBack = { nav.popBackStack() }) }
            composable(Routes.KNOWN_HOSTS) { KnownHostsScreen(shell = shell, onBack = { nav.popBackStack() }) }
            composable(Routes.HISTORY) {
                HistoryScreen(shell = shell, onBack = { nav.popBackStack() }, onOpenHost = { nav.navigate(Routes.hostEdit(it)) })
            }
        }
    }
}

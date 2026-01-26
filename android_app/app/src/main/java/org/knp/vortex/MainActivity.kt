package org.knp.vortex

import android.os.Bundle
import androidx.activity.compose.setContent
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.Button
import androidx.compose.material3.Text
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.navigation.NavHostController
import androidx.navigation.NavType
import androidx.navigation.compose.NavHost
import androidx.navigation.compose.composable
import androidx.navigation.compose.rememberNavController
import androidx.navigation.navArgument
import org.knp.vortex.ui.theme.MediaServerTheme
import org.knp.vortex.ui.screens.home.HomeScreen
import org.knp.vortex.ui.screens.library.ManageLibrariesScreen
import org.knp.vortex.ui.screens.library.CreateLibraryScreen
import org.knp.vortex.ui.screens.library.LibraryScreen
import org.knp.vortex.ui.screens.settings.SettingsScreen
import org.knp.vortex.ui.screens.player.PlayerScreen
import org.knp.vortex.ui.screens.details.MovieDetailScreen
import org.knp.vortex.ui.screens.identify.IdentifyScreen
import org.knp.vortex.ui.screens.series.SeriesDetailScreen
import dagger.hilt.android.AndroidEntryPoint
import java.net.URLEncoder
import java.net.URLDecoder
import java.nio.charset.StandardCharsets

import androidx.fragment.app.FragmentActivity
import androidx.biometric.BiometricManager
import androidx.biometric.BiometricPrompt
import androidx.core.content.ContextCompat
import javax.inject.Inject
import org.knp.vortex.data.repository.SettingsRepository
import java.util.concurrent.Executor
import androidx.activity.viewModels
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.ui.unit.dp
import androidx.compose.material.icons.filled.Home
import androidx.compose.material.icons.filled.Settings
import androidx.compose.material.icons.filled.Search
import androidx.navigation.compose.currentBackStackEntryAsState


@AndroidEntryPoint
class MainActivity : FragmentActivity() {

    // @Inject lateinit var settingsRepository: SettingsRepository // Removed in favor of ViewModel
    
    private val viewModel: MainViewModel by viewModels()

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        
        // Initial Check if enabled
        if (viewModel.isBiometricEnabled() && !viewModel.isAuthenticated.value) {
             authenticate()
        } else {
             viewModel.setAuthenticated(true)
        }

        setContent {
            MediaServerTheme {
                Surface(
                    modifier = Modifier.fillMaxSize(),
                    color = MaterialTheme.colorScheme.background
                ) {
                    val isAuthenticated by viewModel.isAuthenticated.collectAsState()
                    
                    if (isAuthenticated || !viewModel.isBiometricEnabled()) {
                        AppNavigation()
                    } else {
                         // Show Auth Screen or Placeholder while prompt is active
                         Box(Modifier.fillMaxSize(), contentAlignment = androidx.compose.ui.Alignment.Center) {
                              Text("Unlock with Biometrics to continue", color = androidx.compose.ui.graphics.Color.White)
                              Button(onClick = { authenticate() }, modifier = Modifier.padding(top = 16.dp)) {
                                  Text("Touch to Unlock")
                              }
                         }
                    }
                }
            }
        }
    }
    
    override fun onResume() {
        super.onResume()
        if (viewModel.isBiometricEnabled() && !viewModel.isAuthenticated.value) {
            authenticate()
        }
    }

    private fun authenticate() {
        val executor: Executor = ContextCompat.getMainExecutor(this)
        val biometricPrompt = BiometricPrompt(this, executor,
            object : BiometricPrompt.AuthenticationCallback() {
                override fun onAuthenticationSucceeded(result: BiometricPrompt.AuthenticationResult) {
                     super.onAuthenticationSucceeded(result)
                     viewModel.setAuthenticated(true)
                }
                
                override fun onAuthenticationError(errorCode: Int, errString: CharSequence) {
                    super.onAuthenticationError(errorCode, errString)
                    // Handle cancel/error
                }
            })

        val promptInfo = BiometricPrompt.PromptInfo.Builder()
            .setTitle("Vortex Security")
            .setSubtitle("Unlock to access your media")
            .setNegativeButtonText("Cancel")
            .build()
            
        biometricPrompt.authenticate(promptInfo)
    }
}

@Composable
fun AppNavigation() {
    val navController = rememberNavController()
    // Define items for Bottom Nav
    
    // We need current back stack entry to determine selected item
    val navBackStackEntry by navController.currentBackStackEntryAsState()
    val currentDestination = navBackStackEntry?.destination

    androidx.compose.material3.Scaffold(
        containerColor = org.knp.vortex.ui.theme.DeepBackground,
        bottomBar = {
             // Only show bottom nav on main screens
             if (currentDestination?.route in listOf("home", "search", "settings")) {
                 org.knp.vortex.ui.components.GlassyBottomNavigation {
                     org.knp.vortex.ui.components.GlassyBottomNavItem(
                         selected = currentDestination?.route == "home",
                         onClick = { navController.navigate("home") { 
                            popUpTo(navController.graph.startDestinationId) { saveState = true }
                            launchSingleTop = true 
                            restoreState = true
                         } },
                         icon = androidx.compose.material.icons.Icons.Default.Home,
                         label = "Home"
                     )
                     org.knp.vortex.ui.components.GlassyBottomNavItem(
                         selected = currentDestination?.route == "search",
                         onClick = { navController.navigate("search") { 
                            popUpTo(navController.graph.startDestinationId) { saveState = true }
                            launchSingleTop = true 
                            restoreState = true
                         } },
                         icon = androidx.compose.material.icons.Icons.Default.Search,
                         label = "Search"
                     )
                      org.knp.vortex.ui.components.GlassyBottomNavItem(
                         selected = currentDestination?.route == "settings",
                         onClick = { navController.navigate("settings") { 
                            popUpTo(navController.graph.startDestinationId) { saveState = true }
                            launchSingleTop = true 
                            restoreState = true
                         } },
                         icon = androidx.compose.material.icons.Icons.Default.Settings,
                         label = "Settings"
                     )
                 }
             }
        }
    ) { innerPadding ->
        NavHost(
            navController = navController, 
            startDestination = "home",
            modifier = Modifier.padding(innerPadding),
            enterTransition = { androidx.compose.animation.fadeIn() },
            exitTransition = { androidx.compose.animation.fadeOut() },
            popEnterTransition = { androidx.compose.animation.fadeIn() },
            popExitTransition = { androidx.compose.animation.fadeOut() }
        ) {
        composable("home") {
            HomeScreen(
                onPlayMedia = { id, type -> 
                    val t = type?.lowercase() ?: ""
                    if (t == "other" || t == "music_videos") {
                        navController.navigate("player/$id")
                    } else {
                        navController.navigate("movie/$id")
                    }
                },
                onOpenSeries = { name -> 
                    val encoded = URLEncoder.encode(name, StandardCharsets.UTF_8.toString())
                    navController.navigate("series/$encoded/detail")
                },
                onOpenLibrary = { id, name, type -> 
                    val encodedName = URLEncoder.encode(name, StandardCharsets.UTF_8.toString())
                    navController.navigate("library/$id/$encodedName/$type")
                },
                onOpenSettings = { navController.navigate("settings") },
                onQuickPlay = { id -> 
                    // Direct Playback
                    navController.navigate("player/$id") 
                }
            )
        }
        
        composable("search") {
            org.knp.vortex.ui.screens.search.SearchScreen(
                onPlayMedia = { id, type -> 
                    val t = type?.lowercase() ?: ""
                    if (t == "other" || t == "music_videos") {
                        navController.navigate("player/$id")
                    } else {
                        navController.navigate("movie/$id")
                    }
                },
                onOpenSeries = { name -> 
                    val encoded = URLEncoder.encode(name, StandardCharsets.UTF_8.toString())
                    navController.navigate("series/$encoded/detail")
                }
            )
        }
        composable("settings") {
            SettingsScreen(
                onBack = { navController.popBackStack() },
                onManageLibraries = { navController.navigate("manage_libraries") }
            )
        }

        composable("manage_libraries") {
            ManageLibrariesScreen(
                onBack = { navController.popBackStack() },
                onAddLibrary = { navController.navigate("create_library") }
            )
        }

        composable("create_library") {
            CreateLibraryScreen(
                onBack = { navController.popBackStack() },
                onSuccess = { navController.popBackStack() }
            )
        }
        
        composable(
            route = "player/{mediaId}",
            arguments = listOf(navArgument("mediaId") { type = NavType.LongType })
        ) { backStackEntry ->
            val mediaId = backStackEntry.arguments?.getLong("mediaId") ?: return@composable
            PlayerScreen(
                mediaId = mediaId,
                onBack = { navController.popBackStack() }
            )
        }



        composable(
            route = "library/{libId}/{libName}/{libType}",
            arguments = listOf(
                navArgument("libId") { type = NavType.LongType },
                navArgument("libName") { type = NavType.StringType },
                navArgument("libType") { type = NavType.StringType }
            )
        ) { backStackEntry ->
            val libId = backStackEntry.arguments?.getLong("libId") ?: return@composable
            val libName = URLDecoder.decode(backStackEntry.arguments?.getString("libName") ?: "", StandardCharsets.UTF_8.toString())
            val libType = backStackEntry.arguments?.getString("libType") ?: "movies"
            LibraryScreen(
                libraryId = libId,
                libraryName = libName,
                libraryType = libType,
                onPlayMedia = { id, _ -> 
                    val t = libType.lowercase()
                    if (t == "other" || t == "music_videos") {
                        navController.navigate("player/$id")
                    } else {
                        navController.navigate("movie/$id")
                    }
                },
                onOpenSeries = { seriesName ->
                    val encoded = URLEncoder.encode(seriesName, StandardCharsets.UTF_8.toString())
                    navController.navigate("series/$encoded/detail")
                },
                onBack = { navController.popBackStack() }
            )
        }

        composable(
            route = "movie/{mediaId}",
            arguments = listOf(navArgument("mediaId") { type = NavType.LongType })
        ) { backStackEntry ->
            val mediaId = backStackEntry.arguments?.getLong("mediaId") ?: return@composable
            MovieDetailScreen(
                mediaId = mediaId,
                onPlay = { id -> navController.navigate("player/$id") },
                onBack = { navController.popBackStack() },
                onIdentify = { id, title, mediaType ->
                    val encodedTitle = URLEncoder.encode(title ?: "", StandardCharsets.UTF_8.toString())
                    val encodedType = URLEncoder.encode(mediaType ?: "movie", StandardCharsets.UTF_8.toString())
                    navController.navigate("identify/$id/$encodedTitle/$encodedType")
                }
            )
        }

        composable(
            route = "identify/{mediaId}/{title}/{mediaType}?seriesName={seriesName}",
            arguments = listOf(
                navArgument("mediaId") { type = NavType.LongType },
                navArgument("title") { type = NavType.StringType },
                navArgument("mediaType") { type = NavType.StringType },
                navArgument("seriesName") { 
                    type = NavType.StringType
                    nullable = true
                    defaultValue = null
                }
            )
        ) { backStackEntry ->
            val mediaId = backStackEntry.arguments?.getLong("mediaId") ?: return@composable
            val title = URLDecoder.decode(backStackEntry.arguments?.getString("title") ?: "", StandardCharsets.UTF_8.toString())
            val mediaType = URLDecoder.decode(backStackEntry.arguments?.getString("mediaType") ?: "", StandardCharsets.UTF_8.toString())
            val seriesName = backStackEntry.arguments?.getString("seriesName")?.let {
                URLDecoder.decode(it, StandardCharsets.UTF_8.toString())
            }
            IdentifyScreen(
                mediaId = mediaId,
                initialTitle = title,
                mediaType = mediaType,
                seriesName = seriesName,
                onBack = { navController.popBackStack() },
                onIdentified = { navController.popBackStack() }
            )
        }

        composable(
            route = "series/{seriesName}/detail",
            arguments = listOf(navArgument("seriesName") { type = NavType.StringType })
        ) { _ ->
            // Unused val encodedName = backStackEntry.arguments?.getString("seriesName") ?: ""
            // Unused: val seriesName = URLDecoder.decode(encodedName, StandardCharsets.UTF_8.toString())
            SeriesDetailScreen(
                onBack = { navController.popBackStack() },
                onIdentify = { name ->
                    val encodedTitle = URLEncoder.encode(name, StandardCharsets.UTF_8.toString())
                    navController.navigate("identify/0/$encodedTitle/tv?seriesName=$encodedTitle")
                },
                onPlayEpisode = { id -> navController.navigate("player/$id") }
            )
        }
    }
}
}


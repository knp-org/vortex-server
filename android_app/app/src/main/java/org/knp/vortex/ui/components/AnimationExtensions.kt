package org.knp.vortex.ui.components

import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.core.tween
import androidx.compose.foundation.clickable
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.interaction.collectIsPressedAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.ui.Modifier
import androidx.compose.ui.composed
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.animation.core.*
import androidx.compose.foundation.background
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.layout.onGloballyPositioned
import androidx.compose.ui.unit.IntSize
import androidx.compose.runtime.*

fun Modifier.bounceClick(
    scaleDown: Float = 0.95f,
    onClick: (() -> Unit)? = null
) = composed {
    val interactionSource = remember { MutableInteractionSource() }
    val isPressed by interactionSource.collectIsPressedAsState()

    val scale by animateFloatAsState(
        targetValue = if (isPressed) scaleDown else 1f,
        animationSpec = spring(
            dampingRatio = Spring.DampingRatioMediumBouncy,
            stiffness = Spring.StiffnessLow
        ),
        label = "BounceAnimation"
    )

    this
        .graphicsLayer {
            scaleX = scale
            scaleY = scale
        }
        .then(
            if (onClick != null) {
                Modifier.clickable(
                    interactionSource = interactionSource,
                    indication = null, // Disable default ripple if using bounce, or keep it. Let's keep ripple manually if needed, but usually bounce replaces heavy ripple.
                    onClick = onClick
                )
            } else {
                Modifier
            }
        )
}

fun Modifier.shimmerEffect(
    isActive: Boolean = true,
    widthOfShadowBrush: Int = 500,
    angleOfAxisY: Float = 270f,
    durationMillis: Int = 1000
): Modifier = composed {
    if (!isActive) return@composed this

    var size by remember { mutableStateOf(IntSize.Zero) }
    val transition = rememberInfiniteTransition(label = "ShimmerTransition")
    val translateAnimation by transition.animateFloat(
        initialValue = 0f,
        targetValue = size.width.toFloat() + widthOfShadowBrush,
        animationSpec = infiniteRepeatable(
            animation = tween(
                durationMillis = durationMillis,
                easing = LinearEasing
            ),
            repeatMode = RepeatMode.Restart
        ),
        label = "ShimmerTranslate"
    )

    this
        .onGloballyPositioned {
            size = it.size
        }
        .background(
            brush = Brush.linearGradient(
                colors = listOf(
                    Color.White.copy(alpha = 0f),
                    Color.White.copy(alpha = 0.15f), // Subtle shimmer
                    Color.White.copy(alpha = 0f),
                ),
                start = Offset(x = translateAnimation - widthOfShadowBrush, y = 0.0f),
                end = Offset(x = translateAnimation, y = angleOfAxisY),
            )
        )
}

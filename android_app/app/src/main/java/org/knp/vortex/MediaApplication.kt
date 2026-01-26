package org.knp.vortex

import android.app.Application
import dagger.hilt.android.HiltAndroidApp

@HiltAndroidApp
class MediaApplication : Application(), coil.ImageLoaderFactory {
    override fun newImageLoader(): coil.ImageLoader {
        return coil.ImageLoader.Builder(this)
            .components {
                add(coil.decode.VideoFrameDecoder.Factory())
            }
            .crossfade(true)
            .build()
    }
}

package org.knp.vortex.utils

import android.graphics.Bitmap
import android.media.MediaMetadataRetriever
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import java.util.HashMap

object VideoThumbnailHelper {
    private val cache = androidx.collection.LruCache<String, Bitmap>(50) // Cache last 50 thumbnails

    suspend fun extractThumbnail(url: String, timeMicros: Long = 1_000_000L): Bitmap? {
        return withContext(Dispatchers.IO) {
            // Check cache first
            cache.get(url)?.let { return@withContext it }

            val retriever = MediaMetadataRetriever()
            try {
                // Use a HashMap for headers if needed (e.g. Auth)
                val headers = HashMap<String, String>()
                // headers["Authorization"] = "..." // If needed
                
                retriever.setDataSource(url, headers)
                
                // Try to get frame at specified time, fallback to ANY frame if precise fails
                val bitmap = retriever.getFrameAtTime(timeMicros, MediaMetadataRetriever.OPTION_CLOSEST_SYNC) 
                             ?: retriever.getFrameAtTime(timeMicros, MediaMetadataRetriever.OPTION_CLOSEST)
                             ?: retriever.frameAtTime

                if (bitmap != null) {
                    cache.put(url, bitmap)
                }
                bitmap
            } catch (e: Exception) {
                e.printStackTrace()
                null
            } finally {
                try {
                    retriever.release()
                } catch (e: Exception) { }
            }
        }
    }
}

import { AbstractPopupNotificationManager, type PopupNotification } from '@modrinth/ui'
import { type Ref, ref, watch } from 'vue'

import { HIDDEN_ACCOUNT_NAME } from '@/features/plus-privacy/privacy-model'
import { redactForStream, usePrivacy } from '@/features/plus-privacy/use-privacy'

function withoutPrivateDetails(notification: PopupNotification): PopupNotification {
	if (notification.contentType === 'custom' || !usePrivacy().streamerMode.value) return notification
	notification.title = redactForStream(notification.title)
	if (notification.contentType === 'toast') {
		if (notification.actorName) notification.actorName = HIDDEN_ACCOUNT_NAME
		notification.actorAvatarUrl = null
		return notification
	}
	if (notification.text) notification.text = redactForStream(notification.text)
	for (const item of notification.progressItems ?? []) {
		if (item.text) item.text = redactForStream(item.text)
	}
	return notification
}

export class AppPopupNotificationManager extends AbstractPopupNotificationManager {
	private readonly state: Ref<PopupNotification[]>

	public constructor() {
		super()
		this.state = ref<PopupNotification[]>([])
		watch(usePrivacy().streamerMode, (enabled) => {
			if (enabled) this.state.value.forEach(withoutPrivateDetails)
		})
	}

	public getNotifications(): PopupNotification[] {
		return this.state.value
	}

	protected addNotificationToStorage(notification: PopupNotification): void {
		this.state.value.push(withoutPrivateDetails(notification))
	}

	protected removeNotificationFromStorage(id: string | number): void {
		const index = this.state.value.findIndex((n) => n.id === id)
		if (index > -1) {
			this.state.value.splice(index, 1)
		}
	}

	protected clearAllNotificationsFromStorage(): void {
		this.state.value.splice(0)
	}
}

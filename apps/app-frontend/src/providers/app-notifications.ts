import {
	AbstractWebNotificationManager,
	type NotificationPanelLocation,
	type WebNotification,
} from '@modrinth/ui'
import { type Ref, ref, watch } from 'vue'

import { redactForStream, usePrivacy } from '@/features/plus-privacy/use-privacy'

function withoutPrivateDetails<T extends Partial<WebNotification>>(notification: T): T {
	if (typeof notification.title === 'string') notification.title = redactForStream(notification.title)
	if (typeof notification.text === 'string') notification.text = redactForStream(notification.text)
	return notification
}

export class AppNotificationManager extends AbstractWebNotificationManager {
	private readonly state: Ref<WebNotification[]>
	private readonly locationState: Ref<NotificationPanelLocation>

	public constructor() {
		super()
		this.state = ref<WebNotification[]>([])
		this.locationState = ref<NotificationPanelLocation>('right')

		const addNotification = this.addNotification
		this.addNotification = (notification) => addNotification(withoutPrivateDetails(notification))
		watch(usePrivacy().streamerMode, (enabled) => {
			if (enabled) this.state.value.forEach(withoutPrivateDetails)
		})
	}

	public getNotificationLocation(): NotificationPanelLocation {
		return this.locationState.value
	}

	public setNotificationLocation(location: NotificationPanelLocation): void {
		this.locationState.value = location
	}

	public getNotifications(): WebNotification[] {
		return this.state.value
	}

	protected addNotificationToStorage(notification: WebNotification): void {
		this.state.value.push(notification)
	}

	protected removeNotificationFromStorage(id: string | number): void {
		const index = this.state.value.findIndex((n) => n.id === id)
		if (index > -1) {
			this.state.value.splice(index, 1)
		}
	}

	protected removeNotificationFromStorageByIndex(index: number): void {
		this.state.value.splice(index, 1)
	}

	protected clearAllNotificationsFromStorage(): void {
		this.state.value.splice(0)
	}
}
